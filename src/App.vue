<script setup lang="ts">
import { RouterLink, RouterView } from "vue-router";
import { useI18n, type Locale } from "./i18n";

const { locale, setLocale, locales, t } = useI18n();

function onLocaleChange(e: Event) {
  setLocale((e.target as HTMLSelectElement).value as Locale);
}

function openGitHub() {
  try {
    import("@tauri-apps/plugin-shell").then(({ open }) => {
      void open("https://github.com/xfm0797");
    });
  } catch {
    window.open("https://github.com/xfm0797", "_blank");
  }
}
</script>

<template>
  <div class="app">
    <nav class="sidebar">
      <!-- ============ 顶部导航菜单 ============ -->
      <RouterLink to="/">{{ t("nav.posts") }}</RouterLink>
      <RouterLink to="/editor">{{ t("nav.newPost") }}</RouterLink>
      <RouterLink to="/menus">{{ t("nav.menus") }}</RouterLink>
      <RouterLink to="/tags">{{ t("nav.tags") }}</RouterLink>
      <RouterLink to="/themes" class="menu-section-end">{{ t("nav.themes") }}</RouterLink>
      <RouterLink to="/settings">{{ t("nav.settings") }}</RouterLink>

      <!-- ============ 底部：语言切换 + 作者署名 ============ -->
      <div class="sidebar-footer">
        <div class="locale-wrap">
          <select
            class="locale-select"
            :value="locale"
            @change="onLocaleChange"
            :title="t('nav.settings') /* 占位，避免 ESLint 警告未使用 t */"
          >
            <option v-for="l in locales" :key="l.value" :value="l.value">
              {{ l.native }}
            </option>
          </select>
        </div>
        <a
          class="author-link"
          href="https://github.com/xfm0797"
          @click.prevent="openGitHub"
          title="XFM · https://github.com/xfm0797"
        >
          <span class="author-name">XFM</span>
          <span class="author-url">github.com/xfm0797</span>
        </a>
      </div>
    </nav>
    <main class="content">
      <RouterView />
    </main>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
body {
  font-family: system-ui, -apple-system, sans-serif;
}
.app {
  display: flex;
  height: 100vh;
}
.sidebar {
  width: 200px;
  background: #1a1a2e;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.sidebar a {
  text-decoration: none;
  color: #a0a0c0;
  padding: 10px 12px;
  border-radius: 6px;
  font-size: 14px;
  transition: 0.15s;
}
.sidebar a:hover {
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
}
.sidebar a.router-link-active {
  background: rgba(74, 158, 255, 0.15);
  color: #4a9eff;
}
/* 分组分隔：把导航分成"内容管理 / 配置"两组 */
.sidebar a.menu-section-end {
  margin-bottom: 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  padding-bottom: 12px;
}

/* ============ 底部 footer ============ */
.sidebar-footer {
  margin-top: auto;
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.locale-wrap {
  display: flex;
}
.locale-select {
  flex: 1;
  width: 100%;
  padding: 6px 8px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  color: #a0a0c0;
  font-size: 12px;
  cursor: pointer;
  outline: none;
  transition: 0.15s;
}
.locale-select:hover {
  border-color: rgba(74, 158, 255, 0.4);
  color: #fff;
}
.locale-select:focus {
  border-color: rgba(74, 158, 255, 0.6);
}
.locale-select option {
  background: #1a1a2e;
  color: #e0e0e0;
}

.author-link {
  display: flex;
  flex-direction: column;
  padding: 6px 8px;
  background: transparent;
  border-radius: 4px;
  text-decoration: none;
  color: #6b7280;
  transition: 0.15s;
  cursor: pointer;
}
.author-link:hover {
  background: rgba(255, 255, 255, 0.04);
  color: #fff;
}
.author-name {
  font-size: 13px;
  font-weight: 600;
  color: #a0a0c0;
}
.author-link:hover .author-name {
  color: #4a9eff;
}
.author-url {
  font-size: 10px;
  color: #6b7280;
  font-family: "JetBrains Mono", Consolas, monospace;
  margin-top: 1px;
  letter-spacing: -0.2px;
}

.content {
  flex: 1;
  padding: 20px 24px;
  overflow-y: auto;
  background: #fafafa;
}
</style>
