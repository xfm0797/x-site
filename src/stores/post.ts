import { defineStore } from "pinia";
import { ref } from "vue";
import {
  loadPosts,
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
const SAVED_HINT_MS = 2000;

export const usePostStore = defineStore("post", () => {
  // ============================================================
  // 文章列表
  // ============================================================
  const posts = ref<PostMeta[]>([]);
  const loading = ref(false);

  async function fetchPosts() {
    loading.value = true;
    try {
      posts.value = await loadPosts();
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

  // 防抖 timer 句柄
  let debounceTimer: number | null = null;
  let savedHintTimer: number | null = null;

  /** 初始化一篇空白草稿 */
  function initNewPost(id: string) {
    current.value = newDraft(id);
    saveState.value = "idle";
    lastError.value = "";
  }

  /**
   * 标记内容已变更（由 EditorView 的 watch 调用）
   * 触发防抖自动保存。
   */
  function markDirty() {
    if (saveState.value === "saving") return;
    saveState.value = "pending";
    lastError.value = "";

    if (debounceTimer !== null) {
      window.clearTimeout(debounceTimer);
    }
    debounceTimer = window.setTimeout(() => {
      void doSave();
    }, AUTOSAVE_DEBOUNCE_MS);
  }

  /** 真正执行保存 */
  async function doSave(): Promise<void> {
    if (!current.value) return;
    if (saveState.value === "saving") return;

    // 取消未触发的 debounce，避免重复保存
    if (debounceTimer !== null) {
      window.clearTimeout(debounceTimer);
      debounceTimer = null;
    }

    saveState.value = "saving";
    try {
      const post = current.value;
      // 自动刷新 updated_at（同时 Rust 端也会用此值）
      post.updated_at = new Date().toISOString();
      const saved = await savePost(post);
      current.value = saved;
      lastSavedAt.value = new Date();
      saveState.value = "saved";

      // 2 秒后回到 idle，UI 上"已保存"提示消失
      if (savedHintTimer !== null) {
        window.clearTimeout(savedHintTimer);
      }
      savedHintTimer = window.setTimeout(() => {
        saveState.value = "idle";
      }, SAVED_HINT_MS);
    } catch (e: any) {
      lastError.value = String(e?.message ?? e);
      saveState.value = "error";
    }
  }

  /** 手动触发立即保存（用户点击保存按钮） */
  async function saveNow(): Promise<void> {
    // 取消防抖，立即执行
    if (debounceTimer !== null) {
      window.clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    await doSave();
  }

  /** 手动设置发布状态（带保存） */
  async function publish(published: boolean) {
    if (!current.value) return;
    current.value.status = published ? "published" : "draft";
    await saveNow();
  }

  return {
    // 列表
    posts,
    loading,
    fetchPosts,
    // 当前文章
    current,
    saveState,
    lastError,
    lastSavedAt,
    initNewPost,
    markDirty,
    saveNow,
    publish,
  };
});
