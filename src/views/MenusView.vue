<script setup lang="ts">
/**
 * MenusView — 站点导航菜单管理
 *
 * 功能：
 * - 管理站点导航栏菜单项（标题 + URL）
 * - 内置项（首页/归档/友链）可禁用但不能改 URL
 * - 支持新增自定义菜单项（外链、相对路径均可）
 * - 拖拽排序（HTML5 draggable）
 * - 顶部"预览"区显示站点导航栏最终效果
 *
 * 存储：SiteConfig.menus（Rust 端 get_menus / save_menus 持久化），
 * 生成站点时注入模板 `site.menus` 渲染导航。旧版本数据仅存 localStorage，
 * 首次进入且后端可用时自动迁移。
 */
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from "../i18n";
import { getMenus, saveMenus, type MenuItem } from "../api/tauri";

const { t } = useI18n();

/** 后端不可用（浏览器开发环境）时的内置回退项 */
const defaultMenus: MenuItem[] = [
  { label: "首页", url: "/", builtin: true, enabled: true },
  { label: "归档", url: "/archive.html", builtin: true, enabled: true },
  { label: "友情链接", url: "/links.html", builtin: true, enabled: true },
];

const menus = ref<MenuItem[]>([]);
const newLabel = ref("");
const newUrl = ref("");
const message = ref("");
/** 后端是否可用（不可用时退回 localStorage，仅用于浏览器调试） */
const backendAvailable = ref(true);

const STORAGE_KEY = "x-site:menus";
const SAVE_DEBOUNCE_MS = 400;

let saveTimer: number | null = null;
let saveSeq = 0;

function loadFromStorage(): MenuItem[] | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function clearSaveTimer() {
  if (saveTimer !== null) {
    window.clearTimeout(saveTimer);
    saveTimer = null;
  }
}

/** 写入后端（不可用时写 localStorage） */
async function flush() {
  clearSaveTimer();
  if (!backendAvailable.value) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(menus.value));
    return;
  }
  const seq = ++saveSeq;
  try {
    await saveMenus(menus.value);
    if (seq !== saveSeq) return;
    message.value = "已保存";
    setTimeout(() => {
      if (seq === saveSeq) message.value = "";
    }, 2000);
  } catch (e: any) {
    if (seq !== saveSeq) return;
    message.value = `保存失败：${String(e?.message ?? e)}`;
  }
}

/** 防抖持久化：连续输入时只提交最后一次 */
function persist(immediate = false) {
  clearSaveTimer();
  if (immediate) {
    void flush();
    return;
  }
  saveTimer = window.setTimeout(() => {
    saveTimer = null;
    void flush();
  }, SAVE_DEBOUNCE_MS);
}

onMounted(async () => {
  const stored = loadFromStorage();
  try {
    const remote = await getMenus();
    backendAvailable.value = true;
    if (stored && stored.length) {
      // 迁移旧版纯前端数据到后端，成功后删除本地副本
      await saveMenus(stored);
      localStorage.removeItem(STORAGE_KEY);
      menus.value = stored;
    } else {
      menus.value = remote;
    }
  } catch {
    backendAvailable.value = false;
    menus.value = stored ?? defaultMenus;
  }
});

onBeforeUnmount(() => {
  if (saveTimer !== null) {
    clearSaveTimer();
    void flush();
  }
});

/** 当前启用的菜单项（用于预览） */
const enabledMenus = computed(() => menus.value.filter((m) => m.enabled));

/** 新增自定义菜单 */
function addMenu() {
  const label = newLabel.value.trim();
  const url = newUrl.value.trim();
  if (!label || !url) {
    message.value = "请填写标题和链接";
    return;
  }
  menus.value.push({ label, url, builtin: false, enabled: true });
  newLabel.value = "";
  newUrl.value = "";
  persist(true);
}

function removeMenu(index: number) {
  menus.value.splice(index, 1);
  persist(true);
}

function toggleEnabled(index: number) {
  menus.value[index].enabled = !menus.value[index].enabled;
  persist(true);
}

function updateLabel(index: number, label: string) {
  menus.value[index].label = label;
  persist();
}

function updateUrl(index: number, url: string) {
  if (menus.value[index].builtin) return; // 内置不可改 URL
  menus.value[index].url = url;
  persist();
}

// ============================================================
// 拖拽排序
// ============================================================
let dragIndex = -1;

function onDragStart(index: number, e: DragEvent) {
  dragIndex = index;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
  }
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = "move";
  }
}

function onDrop(index: number, e: DragEvent) {
  e.preventDefault();
  if (dragIndex < 0 || dragIndex === index) return;
  const moved = menus.value.splice(dragIndex, 1)[0];
  menus.value.splice(index, 0, moved);
  dragIndex = -1;
  persist();
}
</script>

<template>
  <div class="menus-view">
    <div class="page-header">
      <h1>{{ t("menus.title") }}</h1>
      <span class="hint">{{ t("menus.hint") }}</span>
    </div>

    <!-- 预览：站点导航栏效果 -->
    <section class="preview-section">
      <h2>{{ t("menus.previewTitle") }}</h2>
      <nav class="preview-nav">
        <a v-for="m in enabledMenus" :key="m.url" :href="m.url">{{
          m.label
        }}</a>
        <span v-if="!enabledMenus.length" class="muted">{{ t("menus.noMenus") }}</span>
      </nav>
    </section>

    <!-- 菜单项列表 -->
    <section class="menu-list-section">
      <h2>{{ t("menus.itemsTitle") }}</h2>
      <ul class="menu-list">
        <li
          v-for="(m, i) in menus"
          :key="i"
          draggable="true"
          @dragstart="onDragStart(i, $event)"
          @dragover="onDragOver"
          @drop="onDrop(i, $event)"
        >
          <span class="drag-handle" :title="t('menus.hint')">⋮⋮</span>
          <input
            type="checkbox"
            :checked="m.enabled"
            @change="toggleEnabled(i)"
          />
          <input
            class="input-label"
            :value="m.label"
            @input="updateLabel(i, ($event.target as HTMLInputElement).value)"
            :placeholder="t('menus.labelPlaceholder')"
          />
          <input
            class="input-url"
            :value="m.url"
            :disabled="m.builtin"
            @input="updateUrl(i, ($event.target as HTMLInputElement).value)"
            :placeholder="t('menus.urlPlaceholder')"
          />
          <span v-if="m.builtin" class="builtin-badge" :title="t('menus.builtin')">{{ t("menus.builtin") }}</span>
          <button class="btn-remove" :title="t('editor.meta.coverClear')" @click="removeMenu(i)">×</button>
        </li>
      </ul>
    </section>

    <!-- 新增自定义菜单 -->
    <section class="add-section">
      <h2>{{ t("menus.addTitle") }}</h2>
      <div class="add-form">
        <input
          v-model="newLabel"
          :placeholder="t('menus.labelTitle')"
          @keydown.enter="addMenu"
        />
        <input
          v-model="newUrl"
          :placeholder="t('menus.urlTitle')"
          @keydown.enter="addMenu"
        />
        <button class="btn-primary" @click="addMenu">{{ t("menus.addBtn") }}</button>
      </div>
      <p v-if="message" class="msg">{{ message }}</p>
    </section>
  </div>
</template>

<style scoped>
.menus-view {
  max-width: 800px;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 24px;
}
.page-header h1 {
  font-size: 24px;
  font-weight: 700;
}
.hint {
  font-size: 12px;
  color: #9ca3af;
}

section {
  margin-bottom: 28px;
}
section h2 {
  font-size: 14px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 12px;
}

/* 预览区 */
.preview-section {
  padding: 16px 20px;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
}
.preview-nav {
  display: flex;
  gap: 24px;
  font-size: 14px;
}
.preview-nav a {
  color: #2563eb;
  text-decoration: none;
}
.preview-nav a:hover {
  text-decoration: underline;
}
.muted {
  color: #9ca3af;
  font-size: 13px;
}

/* 菜单列表 */
.menu-list {
  list-style: none;
  padding: 0;
}
.menu-list li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
  margin-bottom: 6px;
  background: #fff;
  transition: all 0.15s;
}
.menu-list li:hover {
  border-color: #93c5fd;
}
.menu-list li[draggable="true"] {
  cursor: move;
}
.drag-handle {
  color: #9ca3af;
  font-size: 14px;
  letter-spacing: -2px;
  cursor: grab;
}
.input-label,
.input-url {
  padding: 6px 10px;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
  font-size: 13px;
  outline: none;
}
.input-label {
  width: 140px;
  font-weight: 500;
}
.input-url {
  flex: 1;
  font-family: "JetBrains Mono", Consolas, monospace;
  color: #6b7280;
}
.input-url:disabled {
  background: #f9fafb;
  color: #9ca3af;
}
.builtin-badge {
  font-size: 11px;
  padding: 2px 8px;
  background: #fef3c7;
  color: #92400e;
  border-radius: 10px;
}
.btn-remove {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: #9ca3af;
  cursor: pointer;
  font-size: 18px;
  line-height: 1;
}
.btn-remove:hover {
  color: #dc2626;
}

/* 新增表单 */
.add-form {
  display: flex;
  gap: 8px;
}
.add-form input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
  font-size: 13px;
  outline: none;
}
.add-form input:focus {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.1);
}
.btn-primary {
  padding: 8px 20px;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-primary:hover {
  background: #1d4ed8;
}

.msg {
  margin-top: 8px;
  font-size: 12px;
  color: #16a34a;
}
</style>
