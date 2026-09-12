<script setup lang="ts">
/**
 * EditorView — 类 Gridea 的 Markdown 编辑器主视图
 *
 * 布局：左 CodeMirror 编辑区 | 右 Markdown 预览 | 右侧浮动元数据面板
 *
 * 功能：
 * - 顶部工具栏：H1/H2、加粗、斜体、链接、图片（dialog 选择）、保存草稿、发布
 * - 拖拽系统图片到编辑器自动复制到 assets/<post_id>/ 并插入 markdown
 * - 编辑停顿 1s 自动保存（防抖由 store 负责），状态栏显示「已保存」
 * - 元数据面板：title / slug / tags / category / cover / status
 */
import { ref, watch, computed, onMounted, onBeforeUnmount, useTemplateRef } from "vue";
import { useRoute, useRouter } from "vue-router";
import CodeMirrorEditor from "../components/editor/CodeMirrorEditor.vue";
import MarkdownPreview from "../components/editor/MarkdownPreview.vue";
import { usePostStore } from "../stores/post";
import { slugify, pickAndImportImage, type Post } from "../api/tauri";
import { useI18n } from "../i18n";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";

const { t } = useI18n();

const route = useRoute();
const router = useRouter();
const store = usePostStore();

// ============================================================
// 初始化文章
// ============================================================
const postId = (route.params.id as string) || crypto.randomUUID();

if (!store.current || store.current.id !== postId) {
  store.initNewPost(postId);
}

// 局部引用 — computed 返回 store.current，用于只读访问和判断
// v-model 直接绑定 store.current.xxx（Pinia store ref 在模板里自动解包）
const draft = computed(() => store.current as Post | null);

// CodeMirror 编辑器实例引用（用于工具栏插入文本）
const editorRef = useTemplateRef<InstanceType<typeof CodeMirrorEditor>>("editorRef");

// appDataDir 用于把图片相对路径转换为 webview 可访问的 URL
const assetsBase = ref<string>("");
onMounted(async () => {
  try {
    assetsBase.value = await appDataDir();
  } catch {
    // 浏览器环境（开发期 Vite）下 appDataDir 不可用
    assetsBase.value = "";
  }
});

// ============================================================
// 内容变更 → 触发自动保存
// ============================================================
function onContentChange(next: string) {
  if (!store.current) return;
  store.current.content = next;
  store.markDirty();
}

// 元数据字段的变更同样触发自动保存
// 注意：直接 v-model 已经在写入 store.current.xxx，
// 这里 watch 的作用是：在字段变化时触发 markDirty 和 slug 自动生成
watch(
  () => store.current?.title,
  (title) => {
    if (!store.current || title === undefined) return;
    // 同步 slug：如果用户没手动改过 slug，自动从 title 生成
    // 这里简化为始终自动同步（Gridea 行为）
    store.current.slug = slugify(title);
    store.markDirty();
  },
);

watch(
  () => store.current?.slug,
  () => {
    if (!store.current) return;
    store.markDirty();
  },
);

watch(
  () => store.current?.category,
  () => {
    if (!store.current) return;
    store.markDirty();
  },
);

watch(
  () => store.current?.cover,
  () => {
    if (!store.current) return;
    store.markDirty();
  },
);

// tags 单独处理（输入是逗号分隔字符串）
const tagsInput = ref<string>(store.current?.tags.join(", ") ?? "");
watch(tagsInput, (val) => {
  if (!store.current) return;
  store.current.tags = val
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);
  store.markDirty();
});

// ============================================================
// 工具栏动作
// ============================================================
function cmdH1() {
  editorRef.value?.prefixLine("# ");
}
function cmdH2() {
  editorRef.value?.prefixLine("## ");
}
function cmdBold() {
  editorRef.value?.wrapSelection("**");
}
function cmdItalic() {
  editorRef.value?.wrapSelection("*");
}

async function cmdInsertLink() {
  const url = window.prompt(t("editor.linkPrompt"));
  if (!url) return;
  editorRef.value?.insertLink(t("editor.toolbar.link"), url);
}

async function cmdInsertImage() {
  const result = (await pickAndImportImage(postId)) ?? [];
  const relPath = result[0];
  const filename = result[1] ?? "image";
  if (!relPath) return;
  editorRef.value?.insertImage(filename.replace(/\.[^.]+$/, ""), relPath);
}

function cmdInsertCover() {
  // 让用户选择一张图片作为封面
  void (async () => {
    const [relPath] = (await pickAndImportImage(postId)) ?? [];
    if (!relPath) return;
    if (store.current) store.current.cover = relPath;
  })();
}

async function cmdSaveDraft() {
  if (store.current) store.current.status = "draft";
  await store.saveNow();
}

async function cmdPublish() {
  if (store.current) store.current.status = "published";
  await store.saveNow();
  // 发布后返回列表
  router.push("/");
}

async function cmdBack() {
  // 离开前确保已保存
  await store.saveNow();
  router.push("/");
}

// ============================================================
// 拖拽图片自动导入
// ============================================================
async function onDrop(e: DragEvent) {
  if (!e.dataTransfer?.files?.length) return;
  e.preventDefault();
  for (const file of Array.from(e.dataTransfer.files)) {
    // 仅处理图片
    if (!file.type.startsWith("image/")) continue;
    // file.path 在 Tauri webview 中可用（标准浏览器为空字符串）
    const absPath = (file as any).path as string | undefined;
    if (!absPath) {
      // 浏览器环境回退：读为 dataURL
      const dataUrl = await readFileAsDataURL(file);
      editorRef.value?.insertImage(file.name, dataUrl);
      continue;
    }
    try {
      // 复制到 assets/<post_id>/，返回相对路径
      const { importImage } = await import("../api/tauri");
      const relPath = await importImage(postId, absPath);
      editorRef.value?.insertImage(file.name.replace(/\.[^.]+$/, ""), relPath);
    } catch (err) {
      console.error("import image failed:", err);
    }
  }
}

function onDragOver(e: DragEvent) {
  // 允许 drop
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = "copy";
  }
}

function readFileAsDataURL(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

// ============================================================
// 预览的图片 URL：把相对路径转为 webview 可访问的 URL
// ============================================================
function resolveImgUrl(src: string): string {
  // 已经是 URL 或 data: 直接返回
  if (/^(https?:|data:)/i.test(src)) return src;
  // 绝对路径
  if (/^[\\/]/.test(src) || /^[a-zA-Z]:/.test(src)) {
    return convertFileSrc(src);
  }
  // 相对路径 → 基于 appDataDir 解析
  if (assetsBase.value) {
    const abs = `${assetsBase.value}/${src}`.replace(/\\/g, "/");
    return convertFileSrc(abs);
  }
  return src;
}

// ============================================================
// 预览内容（用 computed 把 markdown 源传给 MarkdownPreview）
// 同时把图片相对路径预先转换为 URL，让预览正常显示
// ============================================================
const previewSource = computed(() => {
  if (!store.current) return "";
  // 把 markdown 中的图片相对路径预先替换为 webview URL
  return store.current.content.replace(
    /!\[([^\]]*)\]\(([^)]+)\)/g,
    (_m, alt: string, src: string) => {
      return `![${alt}](${resolveImgUrl(src)})`;
    },
  );
});

// ============================================================
// 离开前保存（用户切走标签 / 关闭窗口时）
// ============================================================
function beforeUnload() {
  // 同步触发保存（不等 await）
  void store.saveNow();
}
onMounted(() => {
  window.addEventListener("beforeunload", beforeUnload);
});
onBeforeUnmount(() => {
  window.removeEventListener("beforeunload", beforeUnload);
});

// ============================================================
// 状态栏文案
// ============================================================
const statusBarText = computed(() => {
  switch (store.saveState) {
    case "idle":
      return t("editor.status.idle");
    case "pending":
      return t("editor.status.pending");
    case "saving":
      return t("editor.status.saving");
    case "saved":
      return t("editor.status.saved");
    case "error":
      return t("editor.status.error", { msg: store.lastError });
    default:
      return "";
  }
});
</script>

<template>
  <div class="editor-view" @drop="onDrop" @dragover="onDragOver">
    <!-- ============ 顶部工具栏 ============ -->
    <header class="toolbar">
      <div class="toolbar-group">
        <button class="btn-icon" :title="t('editor.toolbar.h1')" @click="cmdH1">H1</button>
        <button class="btn-icon" :title="t('editor.toolbar.h2')" @click="cmdH2">H2</button>
        <button class="btn-icon" :title="t('editor.toolbar.bold')" @click="cmdBold">B</button>
        <button class="btn-icon" :title="t('editor.toolbar.italic')" @click="cmdItalic"><i>I</i></button>
      </div>
      <div class="toolbar-group">
        <button class="btn-icon" @click="cmdInsertLink" :title="t('editor.toolbar.link')">🔗</button>
        <button class="btn-icon" @click="cmdInsertImage" :title="t('editor.toolbar.image')">🖼</button>
        <button class="btn-icon" @click="cmdInsertCover" :title="t('editor.toolbar.cover')">⭐</button>
      </div>
      <div class="toolbar-group push-right">
        <button class="btn-text btn-secondary" @click="cmdBack">{{ t("editor.toolbar.back") }}</button>
        <button class="btn-text" @click="cmdSaveDraft">{{ t("editor.toolbar.saveDraft") }}</button>
        <button
          class="btn-text btn-primary"
          :disabled="!draft?.title"
          @click="cmdPublish"
        >
          {{ t("editor.toolbar.publish") }}
        </button>
      </div>
    </header>

    <!-- ============ 主体：编辑 + 预览 ============ -->
    <div class="editor-body">
      <div class="editor-pane">
        <CodeMirrorEditor
          ref="editorRef"
          :model-value="draft?.content ?? ''"
          @update:model-value="onContentChange"
        />
      </div>
      <div class="preview-pane">
        <MarkdownPreview :source="previewSource" />
      </div>

      <!-- ============ 右侧元数据面板 ============ -->
      <aside v-if="store.current" class="meta-panel">
        <h3 class="meta-title">{{ t("editor.meta.title") }}</h3>

        <label class="field">
          <span>{{ t("editor.meta.titleLabel") }}</span>
          <input
            v-model="store.current.title"
            type="text"
            :placeholder="t('editor.meta.titlePlaceholder')"
            class="input"
          />
        </label>

        <label class="field">
          <span>{{ t("editor.meta.slugLabel") }}</span>
          <input
            v-model="store.current.slug"
            type="text"
            :placeholder="t('editor.meta.slugPlaceholder')"
            class="input"
          />
        </label>

        <label class="field">
          <span>{{ t("editor.meta.categoryLabel") }}</span>
          <input
            v-model="store.current.category"
            type="text"
            :placeholder="t('editor.meta.categoryPlaceholder')"
            class="input"
          />
        </label>

        <label class="field">
          <span>{{ t("editor.meta.tagsLabel") }}</span>
          <input
            v-model="tagsInput"
            type="text"
            :placeholder="t('editor.meta.tagsPlaceholder')"
            class="input"
          />
        </label>

        <div class="field">
          <span>{{ t("editor.meta.coverLabel") }}</span>
          <div class="cover-area">
            <img
              v-if="draft?.cover"
              :src="resolveImgUrl(draft.cover)"
              alt="cover"
              class="cover-preview"
            />
            <div v-else class="cover-empty">{{ t("editor.meta.coverEmpty") }}</div>
          </div>
          <div class="cover-actions">
            <button class="btn-mini" @click="cmdInsertCover">{{ t("editor.meta.coverSelect") }}</button>
            <button
              v-if="draft?.cover"
              class="btn-mini btn-mini-danger"
              @click="store.current && (store.current.cover = '')"
            >
              {{ t("editor.meta.coverClear") }}
            </button>
          </div>
        </div>

        <label class="field">
          <span>{{ t("editor.meta.statusLabel") }}</span>
          <select v-model="store.current.status" class="input">
            <option value="draft">{{ t("editor.meta.statusDraft") }}</option>
            <option value="published">{{ t("editor.meta.statusPublished") }}</option>
          </select>
        </label>
      </aside>
    </div>

    <!-- ============ 底部状态栏 ============ -->
    <footer class="status-bar">
      <span class="status-text">{{ statusBarText }}</span>
      <span v-if="store.lastSavedAt" class="status-time">
        最近保存：{{ store.lastSavedAt.toLocaleTimeString() }}
      </span>
    </footer>
  </div>
</template>

<style scoped>
.editor-view {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #f9fafb;
}

/* ============ 工具栏 ============ */
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: #fff;
  border-bottom: 1px solid #e5e7eb;
  flex-shrink: 0;
}
.toolbar-group {
  display: flex;
  gap: 4px;
}
.toolbar-group.push-right {
  margin-left: auto;
}
.btn-icon {
  width: 32px;
  height: 32px;
  border: 1px solid #e5e7eb;
  background: #fff;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  color: #374151;
  transition: all 0.15s;
}
.btn-icon:hover {
  background: #f3f4f6;
  border-color: #d1d5db;
}
.btn-icon:active {
  background: #e5e7eb;
}
.btn-text {
  padding: 6px 14px;
  font-size: 13px;
  border: 1px solid #e5e7eb;
  background: #fff;
  border-radius: 4px;
  cursor: pointer;
  color: #374151;
  transition: all 0.15s;
}
.btn-text:hover {
  background: #f3f4f6;
}
.btn-text.btn-primary {
  background: #2563eb;
  color: #fff;
  border-color: #2563eb;
}
.btn-text.btn-primary:hover {
  background: #1d4ed8;
}
.btn-text.btn-primary:disabled {
  background: #9ca3af;
  border-color: #9ca3af;
  cursor: not-allowed;
}
.btn-text.btn-secondary {
  background: transparent;
}

/* ============ 主体三栏 ============ */
.editor-body {
  display: grid;
  grid-template-columns: 1fr 1fr 280px;
  flex: 1;
  min-height: 0;
}
.editor-pane,
.preview-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid #e5e7eb;
}
.preview-pane {
  border-right: 1px solid #e5e7eb;
  background: #fff;
}

/* ============ 元数据面板 ============ */
.meta-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  background: #fff;
  overflow: auto;
}
.meta-title {
  font-size: 14px;
  font-weight: 600;
  color: #6b7280;
  margin: 0 0 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: #6b7280;
}
.input {
  padding: 6px 10px;
  font-size: 13px;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
  outline: none;
  background: #fff;
  color: #111827;
}
.input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.1);
}
.cover-area {
  width: 100%;
  aspect-ratio: 16 / 9;
  border: 1px dashed #d1d5db;
  border-radius: 4px;
  overflow: hidden;
  background: #f9fafb;
  display: flex;
  align-items: center;
  justify-content: center;
}
.cover-preview {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.cover-empty {
  color: #9ca3af;
  font-size: 12px;
}
.cover-actions {
  display: flex;
  gap: 6px;
  margin-top: 6px;
}
.btn-mini {
  padding: 4px 10px;
  font-size: 12px;
  border: 1px solid #e5e7eb;
  background: #fff;
  border-radius: 3px;
  cursor: pointer;
  color: #374151;
}
.btn-mini:hover {
  background: #f3f4f6;
}
.btn-mini-danger {
  color: #dc2626;
  border-color: #fecaca;
}
.btn-mini-danger:hover {
  background: #fef2f2;
}

/* ============ 状态栏 ============ */
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 16px;
  background: #f3f4f6;
  border-top: 1px solid #e5e7eb;
  font-size: 12px;
  color: #6b7280;
  flex-shrink: 0;
}
.status-text {
  font-weight: 500;
}
</style>
