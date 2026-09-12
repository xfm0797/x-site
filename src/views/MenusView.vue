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
 * 存储：复用 site_config 表中的 menus 字段（前端调用 save_menus 命令），
 * Rust 端 site_config.menus: Vec<MenuItem> 需要扩展，当前实现仅前端状态演示，
 * 后续接入 Rust 后端即可持久化。
 */
import { ref, computed, onMounted } from "vue";
import { useI18n } from "../i18n";

const { t } = useI18n();

interface MenuItem {
  /** 显示标题 */
  label: string;
  /** 链接地址（相对路径或绝对 URL） */
  url: string;
  /** 是否内置项（首页/归档/友链等，不可删除 URL） */
  builtin?: boolean;
  /** 是否启用 */
  enabled: boolean;
}

/** 内置菜单项（对应已生成的页面） */
const defaultMenus: MenuItem[] = [
  { label: "首页", url: "/", builtin: true, enabled: true },
  { label: "归档", url: "/archive.html", builtin: true, enabled: true },
  { label: "友情链接", url: "/links.html", builtin: true, enabled: false },
];

const menus = ref<MenuItem[]>(loadFromStorage() ?? defaultMenus);
const newLabel = ref("");
const newUrl = ref("");
const message = ref("");

const STORAGE_KEY = "x-site:menus";

function loadFromStorage(): MenuItem[] | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function persist() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(menus.value));
}

onMounted(() => {
  if (!localStorage.getItem(STORAGE_KEY)) {
    persist();
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
  menus.value.push({ label, url, enabled: true });
  newLabel.value = "";
  newUrl.value = "";
  persist();
  message.value = "已添加";
  setTimeout(() => (message.value = ""), 2000);
}

function removeMenu(index: number) {
  menus.value.splice(index, 1);
  persist();
}

function toggleEnabled(index: number) {
  menus.value[index].enabled = !menus.value[index].enabled;
  persist();
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
