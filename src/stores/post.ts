import { defineStore } from "pinia";
import { ref } from "vue";
import {
  loadPosts,
  loadPost,
  deletePost,
  savePost,
  newDraft,
  type Post,
  type PostMeta,
} from "../api/tauri";

/**
 * 自动保存状态机
 * - idle: 当前没有待保存的更改
 * - pending: 编辑停顿 debounce 中，尚未到时间
 * - saving: 正在调用 save_post
 * - saved: 保存完成，会在 2s 后回到 idle
 * - error: 保存失败，等待下次编辑触发重试
 */
export type SaveState = "idle" | "pending" | "saving" | "saved" | "error";

const AUTOSAVE_DEBOUNCE_MS = 1000;
const RESAVE_DELAY_MS = 300;
const SAVED_HINT_MS = 2000;

export const usePostStore = defineStore("post", () => {
  // ============================================================
  // 文章列表
  // ============================================================
  const posts = ref<PostMeta[]>([]);
  const loading = ref(false);
  const listError = ref("");

  async function fetchPosts() {
    loading.value = true;
    listError.value = "";
    try {
      posts.value = await loadPosts();
    } catch (e: any) {
      // 加载失败必须显式暴露，避免被误认为"空列表"
      listError.value = String(e?.message ?? e);
    } finally {
      loading.value = false;
    }
  }

  // ============================================================
  // 当前正在编辑的文章
  // ============================================================
  const current = ref<Post | null>(null);
  const saveState = ref<SaveState>("idle");
  const lastError = ref<string>("");
  const lastSavedAt = ref<Date | null>(null);
  const loadingPost = ref(false);

  // 防抖 timer 句柄
  let debounceTimer: number | null = null;
  let savedHintTimer: number | null = null;
  // 编辑版本号：保存期间的新编辑会使 revision 增加，用于避免旧快照覆盖新输入
  let revision = 0;
  // 进行中的保存 Promise：并发的保存请求共享同一次调用，不并发写库
  let inflight: Promise<void> | null = null;

  function clearDebounce() {
    if (debounceTimer !== null) {
      window.clearTimeout(debounceTimer);
      debounceTimer = null;
    }
  }

  /** 初始化一篇空白草稿 */
  function initNewPost(id: string) {
    clearDebounce();
    current.value = newDraft(id);
    revision++;
    saveState.value = "idle";
    lastError.value = "";
    lastSavedAt.value = null;
  }

  /**
   * 加载已有文章（含正文）。
   * 加载失败时清空 current 并抛错，杜绝编辑器用空白草稿自动保存覆盖原文。
   */
  async function loadExisting(id: string): Promise<void> {
    clearDebounce();
    // 避免加载期间短暂显示上一篇文章的内容
    if (current.value?.id !== id) current.value = null;
    loadingPost.value = true;
    lastError.value = "";
    try {
      const post = await loadPost(id);
      current.value = post;
      revision++;
      saveState.value = "idle";
      lastSavedAt.value = null;
    } catch (e) {
      current.value = null;
      saveState.value = "idle";
      throw e;
    } finally {
      loadingPost.value = false;
    }
  }

  /**
   * 标记内容已变更（由 EditorView 的 watch 调用）
   * 触发防抖自动保存。
   */
  function markDirty() {
    if (!current.value) return;
    revision++;
    lastError.value = "";
    if (saveState.value === "saving") {
      // 保存进行中：仅记录版本号，保存完成后由 doSave 按 revision 补存
      return;
    }
    saveState.value = "pending";
    clearDebounce();
    debounceTimer = window.setTimeout(() => {
      void doSave();
    }, AUTOSAVE_DEBOUNCE_MS);
  }

  /** 真正执行保存 */
  function doSave(): Promise<void> {
    if (!current.value) return Promise.resolve();
    // 已有保存进行中：共享同一 Promise，等待其完成而非并发写库
    if (saveState.value === "saving" && inflight) return inflight;

    clearDebounce();
    saveState.value = "saving";
    const startRevision = revision;

    inflight = (async () => {
      try {
        // 深拷贝快照：保存期间的后续输入不会污染本次提交内容
        const snapshot: Post = JSON.parse(JSON.stringify(current.value));
        // 自动刷新 updated_at（同时 Rust 端也会用此值）
        snapshot.updated_at = new Date().toISOString();
        const saved = await savePost(snapshot);
        lastSavedAt.value = new Date();

        if (current.value && current.value.id === saved.id && revision === startRevision) {
          // 保存期间无新编辑：安全采用服务端返回值
          current.value = saved;
          saveState.value = "saved";

          // 2 秒后回到 idle，UI 上"已保存"提示消失
          if (savedHintTimer !== null) {
            window.clearTimeout(savedHintTimer);
          }
          savedHintTimer = window.setTimeout(() => {
            saveState.value = "idle";
          }, SAVED_HINT_MS);
        } else {
          // 保存期间又有输入：保留新内容（绝不用旧快照覆盖），延迟补存
          saveState.value = "pending";
          clearDebounce();
          debounceTimer = window.setTimeout(() => {
            void doSave();
          }, RESAVE_DELAY_MS);
        }
      } catch (e: any) {
        lastError.value = String(e?.message ?? e);
        saveState.value = "error";
      } finally {
        inflight = null;
      }
    })();

    return inflight;
  }

  /** 立即保存并尽量冲刷到干净状态（手动保存 / 路由离开时调用） */
  async function saveNow(): Promise<void> {
    // 保存期间持续输入时最多补存 5 轮，避免无限等待
    for (let i = 0; i < 5; i++) {
      clearDebounce();
      const before = revision;
      await doSave();
      if (revision === before) return;
    }
  }

  /** 手动设置发布状态（带保存） */
  async function publish(published: boolean) {
    if (!current.value) return;
    current.value.status = published ? "published" : "draft";
    await saveNow();
  }

  /** 删除文章并同步列表与编辑状态 */
  async function removePost(id: string): Promise<void> {
    await deletePost(id);
    posts.value = posts.value.filter((p) => p.id !== id);
    if (current.value?.id === id) {
      clearDebounce();
      current.value = null;
      saveState.value = "idle";
    }
  }

  return {
    // 列表
    posts,
    loading,
    listError,
    fetchPosts,
    removePost,
    // 当前文章
    current,
    saveState,
    lastError,
    lastSavedAt,
    loadingPost,
    initNewPost,
    loadExisting,
    markDirty,
    saveNow,
    publish,
  };
});
