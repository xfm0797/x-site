<script setup lang="ts">
/**
 * ThemeView — 主题管理视图
 *
 * 功能：
 * - 网格列出所有已安装主题（截图 + 名称 + 作者 + 描述）
 * - 当前激活主题有标记
 * - 一键切换主题（applyTheme）
 * - 从本地 zip 文件导入新主题（importTheme）
 * - 截图通过 convertFileSrc 转为 webview URL（主题位于 app_data_dir/themes/）
 */
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  listThemes,
  applyTheme,
  importTheme,
  type Theme,
} from "../api/tauri";
import { useI18n } from "../i18n";

const { t } = useI18n();

const themes = ref<Theme[]>([]);
const loading = ref(false);
const message = ref("");
const importing = ref(false);

let unlistenThemeChanged: UnlistenFn | null = null;

// app_data_dir 用于把主题截图的相对路径转成 webview URL
const dataDir = ref<string>("");

onMounted(async () => {
  loading.value = true;
  try {
    dataDir.value = await appDataDir();
    await fetchThemes();
    // 监听主题切换事件（apply_theme 后 Rust 端 emit）
    unlistenThemeChanged = await listen<string>("theme://changed", () => {
      void fetchThemes();
    });
  } finally {
    loading.value = false;
  }
});

onBeforeUnmount(() => {
  unlistenThemeChanged?.();
});

async function fetchThemes() {
  try {
    themes.value = await listThemes();
  } catch (e) {
    message.value = `加载失败：${e}`;
  }
}

const activeTheme = computed(() => themes.value.find((t) => t.active));

/** 切换主题 */
async function switchTheme(theme: Theme) {
  if (theme.active) return;
  try {
    await applyTheme(theme.name);
    message.value = `已切换到主题：${theme.display_name}`;
    await fetchThemes();
  } catch (e) {
    message.value = `切换失败：${e}`;
  }
}

/** 导入 zip 主题包 */
async function pickZip() {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "Theme ZIP", extensions: ["zip"] }],
  });
  if (!selected) return;
  const zipPath = selected as string;

  importing.value = true;
  message.value = "正在导入...";
  try {
    const themeName = await importTheme(zipPath);
    message.value = `导入成功：${themeName}`;
    await fetchThemes();
  } catch (e) {
    message.value = `导入失败：${e}`;
  } finally {
    importing.value = false;
  }
}

/** 解析截图 URL：把主题目录的相对路径转为 webview 可访问的绝对 URL */
function screenshotUrl(theme: Theme): string {
  if (!theme.screenshot || !dataDir.value) return "";
  // 路径形如：themes/<name>/screenshot.png
  const abs = `${dataDir.value}/themes/${theme.name}/${theme.screenshot}`;
  return convertFileSrc(abs);
}

/** 截图加载失败时的占位（用主题名首字母作为头像） */
function onImgError(e: Event) {
  const img = e.target as HTMLImageElement;
  img.style.display = "none";
}
</script>

<template>
  <div class="themes-view">
    <header class="page-header">
      <h1>{{ t("themes.title") }}</h1>
      <button class="btn-import" :disabled="importing" @click="pickZip">
        {{ importing ? t("themes.importing") : t("themes.import") }}
      </button>
    </header>

    <!-- 当前主题 -->
    <div v-if="activeTheme" class="active-banner">
      <span class="label">{{ t("themes.current") }}</span>
      <strong>{{ activeTheme.display_name }}</strong>
      <span class="author">{{ t("themes.by", { author: activeTheme.author }) }}</span>
    </div>

    <!-- 主题网格 -->
    <div v-if="loading" class="loading">{{ t("themes.loading") }}</div>
    <div v-else class="theme-grid">
      <div
        v-for="theme in themes"
        :key="theme.name"
        class="theme-card"
        :class="{ active: theme.active }"
        @click="switchTheme(theme)"
      >
        <!-- 截图 -->
        <div class="card-cover">
          <img
            v-if="screenshotUrl(theme)"
            :src="screenshotUrl(theme)"
            :alt="theme.display_name"
            @error="onImgError"
          />
          <div v-else class="cover-placeholder">
            {{ theme.display_name.charAt(0).toUpperCase() }}
          </div>
        </div>

        <!-- 信息 -->
        <div class="card-body">
          <div class="card-head">
            <h3>{{ theme.display_name }}</h3>
            <span v-if="theme.active" class="active-badge">{{ t("themes.active") }}</span>
          </div>
          <p class="author">{{ theme.author }} · v{{ theme.version }}</p>
          <p class="desc">{{ theme.description }}</p>
        </div>
      </div>
    </div>

    <p v-if="message" class="msg">{{ message }}</p>
  </div>
</template>

<style scoped>
.themes-view {
  padding: 20px 24px;
  max-width: 1200px;
  margin: 0 auto;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}
.page-header h1 {
  font-size: 24px;
  font-weight: 700;
}

.btn-import {
  padding: 8px 16px;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-import:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.active-banner {
  padding: 10px 16px;
  background: #eff6ff;
  border: 1px solid #bfdbfe;
  border-radius: 4px;
  margin-bottom: 20px;
  font-size: 14px;
}
.active-banner .label {
  color: #6b7280;
  margin-right: 6px;
}
.active-banner .author {
  color: #6b7280;
  margin-left: 8px;
}

.loading {
  padding: 40px;
  text-align: center;
  color: #6b7280;
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 20px;
}

.theme-card {
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.2s;
  background: #fff;
}
.theme-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  transform: translateY(-2px);
  border-color: #93c5fd;
}
.theme-card.active {
  border-color: #2563eb;
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.2);
}

.card-cover {
  aspect-ratio: 16 / 10;
  background: linear-gradient(135deg, #f3f4f6, #e5e7eb);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}
.card-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.cover-placeholder {
  font-size: 48px;
  font-weight: 700;
  color: #9ca3af;
}

.card-body {
  padding: 12px 16px;
}
.card-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}
.card-head h3 {
  font-size: 16px;
  font-weight: 600;
}
.active-badge {
  font-size: 11px;
  padding: 2px 8px;
  background: #2563eb;
  color: #fff;
  border-radius: 10px;
}
.author {
  font-size: 12px;
  color: #6b7280;
  margin-bottom: 8px;
}
.desc {
  font-size: 13px;
  color: #4b5563;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.msg {
  margin-top: 20px;
  padding: 10px 14px;
  background: #f3f4f6;
  border-radius: 4px;
  font-size: 13px;
}
</style>
