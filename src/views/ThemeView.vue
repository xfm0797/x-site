<script setup lang="ts">
/**
 * ThemeView — 主题管理 + 通用设置（Tab 切换）
 *
 * Tab「主题」：网格列出/切换/导入主题
 * Tab「设置」：网站名称/描述 + Logo/页头页尾/广告/评论/备案友链
 */
import { ref, reactive, onMounted, onBeforeUnmount, computed } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  listThemes,
  applyTheme,
  importTheme,
  getThemeSettings,
  saveThemeSettings,
  pickAndUploadLogo,
  type Theme,
  type SiteConfig,
  type CommentSystem,
} from "../api/tauri";
import { useI18n } from "../i18n";

const { t } = useI18n();

// ============================================================
// Tab 状态
// ============================================================
const activeTab = ref<"themes" | "settings">("themes");

// ============================================================
// 主题 Tab 状态
// ============================================================
const themes = ref<Theme[]>([]);
const themeLoading = ref(false);
const themeMsg = ref("");
const importing = ref(false);
let unlistenThemeChanged: UnlistenFn | null = null;

const dataDir = ref<string>("");

async function fetchThemes() {
  try {
    themes.value = await listThemes();
  } catch (e) {
    themeMsg.value = `加载失败：${e}`;
  }
}

const activeTheme = computed(() => themes.value.find((t) => t.active));

async function switchTheme(theme: Theme) {
  if (theme.active) return;
  try {
    await applyTheme(theme.name);
    themeMsg.value = `已切换到主题：${theme.display_name}`;
    await fetchThemes();
  } catch (e) {
    themeMsg.value = `切换失败：${e}`;
  }
}

async function pickZip() {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "Theme ZIP", extensions: ["zip"] }],
  });
  if (!selected) return;
  const zipPath = selected as string;

  importing.value = true;
  themeMsg.value = "正在导入...";
  try {
    const themeName = await importTheme(zipPath);
    themeMsg.value = `导入成功：${themeName}`;
    await fetchThemes();
  } catch (e) {
    themeMsg.value = `导入失败：${e}`;
  } finally {
    importing.value = false;
  }
}

function screenshotUrl(theme: Theme): string {
  if (!theme.screenshot || !dataDir.value) return "";
  const abs = `${dataDir.value}/themes/${theme.name}/${theme.screenshot}`;
  return convertFileSrc(abs);
}

function onImgError(e: Event) {
  const img = e.target as HTMLImageElement;
  img.style.display = "none";
}

// ============================================================
// 设置 Tab 状态（合并自 ThemeSettingsView）
// ============================================================
const settingsLoading = ref(false);
const saving = ref(false);
const settingsMsg = ref("");

const form = reactive<SiteConfig>({
  title: "",
  description: "",
  author: "",
  url: "",
  theme: "default",
  posts_per_page: 10,
  links: [],
  logo: "",
  header_html: "",
  footer_html: "",
  ads: {
    header_html: "",
    sidebar_html: "",
    post_footer_html: "",
    site_footer_html: "",
  },
  comments: {
    system: "none",
    params: {},
  },
  icp: "",
  deploy_targets: [],
});

// 评论系统动态字段
interface CommentFieldDef {
  key: string;
  label: string;
  placeholder: string;
}

const commentFields: Record<CommentSystem, CommentFieldDef[]> = {
  none: [],
  disqus: [{ key: "shortname", label: "Shortname", placeholder: "myblog" }],
  changyan: [
    { key: "appid", label: "App ID", placeholder: "cyvr..." },
    { key: "conf", label: "Conf", placeholder: "prod..." },
  ],
  livere: [
    { key: "uid", label: "UID", placeholder: "MTAy..." },
    { key: "site", label: "Site", placeholder: "myblog" },
  ],
  valine: [
    { key: "appid", label: "App ID", placeholder: "..." },
    { key: "appkey", label: "App Key", placeholder: "..." },
  ],
  utterances: [
    { key: "repo", label: "Repo", placeholder: "owner/name" },
    { key: "label", label: "Label", placeholder: "blog" },
    { key: "theme", label: "Theme", placeholder: "github-light" },
  ],
  giscus: [
    { key: "repo", label: "Repo", placeholder: "owner/name" },
    { key: "category", label: "Category", placeholder: "Announcements" },
    { key: "mapping", label: "Mapping", placeholder: "pathname" },
  ],
  custom: [
    { key: "html", label: "Custom HTML", placeholder: "<script>...</" + "script>" },
  ],
};

const currentCommentFields = computed(() =>
  commentFields[form.comments.system] ?? [],
);

function getParam(key: string): string {
  return form.comments.params[key] ?? "";
}
function setParam(key: string, val: string) {
  form.comments.params[key] = val;
}

const logoUrl = computed(() => {
  if (!form.logo || !dataDir.value) return "";
  return convertFileSrc(`${dataDir.value}/${form.logo}`);
});

async function handleSave() {
  saving.value = true;
  settingsMsg.value = "";
  try {
    await saveThemeSettings(form);
    settingsMsg.value = t("themeSettings.saved");
  } catch (e) {
    settingsMsg.value = `${t("common.error")}: ${e}`;
  } finally {
    saving.value = false;
  }
}

async function handleUploadLogo() {
  try {
    const result = await pickAndUploadLogo();
    if (!result) return;
    const [relPath] = result;
    form.logo = relPath;
  } catch (e) {
    settingsMsg.value = `Logo 上传失败：${e}`;
  }
}

function handleRemoveLogo() {
  form.logo = "";
}

function addLink() {
  form.links.push({ name: "", url: "", description: "" });
}
function removeLink(i: number) {
  form.links.splice(i, 1);
}

// ============================================================
// 生命周期
// ============================================================
onMounted(async () => {
  themeLoading.value = true;
  settingsLoading.value = true;
  try {
    dataDir.value = await appDataDir();
    await fetchThemes();
    unlistenThemeChanged = await listen<string>("theme://changed", () => {
      void fetchThemes();
    });

    // 加载设置 Tab 数据
    const cfg = await getThemeSettings();
    Object.assign(form, cfg);
    if (!form.ads) form.ads = { header_html: "", sidebar_html: "", post_footer_html: "", site_footer_html: "" };
    if (!form.comments) form.comments = { system: "none", params: {} };
    if (!form.links) form.links = [];
    if (!form.deploy_targets) form.deploy_targets = [];
  } catch (e) {
    settingsMsg.value = `加载失败：${e}`;
  } finally {
    themeLoading.value = false;
    settingsLoading.value = false;
  }
});

onBeforeUnmount(() => {
  unlistenThemeChanged?.();
});
</script>

<template>
  <div class="themes-view">
    <!-- Tab 栏 -->
    <div class="tabs">
      <button
        class="tab-btn"
        :class="{ active: activeTab === 'themes' }"
        @click="activeTab = 'themes'"
      >
        {{ t("themes.title") }}
      </button>
      <button
        class="tab-btn"
        :class="{ active: activeTab === 'settings' }"
        @click="activeTab = 'settings'"
      >
        {{ t("themeSettings.title") }}
      </button>
    </div>

    <!-- ==================== Tab: 主题 ==================== -->
    <div v-show="activeTab === 'themes'">
      <header class="page-header">
        <button class="btn-import" :disabled="importing" @click="pickZip">
          {{ importing ? t("themes.importing") : t("themes.import") }}
        </button>
      </header>

      <div v-if="activeTheme" class="active-banner">
        <span class="label">{{ t("themes.current") }}</span>
        <strong>{{ activeTheme.display_name }}</strong>
        <span class="author">{{ t("themes.by", { author: activeTheme.author }) }}</span>
      </div>

      <div v-if="themeLoading" class="loading">{{ t("themes.loading") }}</div>
      <div v-else class="theme-grid">
        <div
          v-for="theme in themes"
          :key="theme.name"
          class="theme-card"
          :class="{ active: theme.active }"
          @click="switchTheme(theme)"
        >
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
      <p v-if="themeMsg" class="msg">{{ themeMsg }}</p>
    </div>

    <!-- ==================== Tab: 设置 ==================== -->
    <div v-show="activeTab === 'settings'" class="settings-tab">
      <header class="page-header">
        <button class="btn-save" :disabled="saving || settingsLoading" @click="handleSave">
          {{ saving ? t("themeSettings.saving") : t("themeSettings.save") }}
        </button>
      </header>

      <div v-if="settingsLoading" class="loading">{{ t("home.loading") }}</div>
      <template v-else>
        <!-- 网站名称/描述 -->
        <section class="card">
          <h2>{{ t("themeSettings.siteInfo") }}</h2>
          <label class="field-label">{{ t("themeSettings.siteName") }}</label>
          <input v-model="form.title" class="input" :placeholder="t('themeSettings.siteNamePlaceholder')" />

          <label class="field-label">{{ t("themeSettings.siteDesc") }}</label>
          <textarea v-model="form.description" class="code-area" rows="3" :placeholder="t('themeSettings.siteDescPlaceholder')"></textarea>

          <label class="field-label">{{ t("themeSettings.siteUrl") }}</label>
          <input v-model="form.url" class="input" placeholder="https://example.com" />

          <label class="field-label">{{ t("themeSettings.siteAuthor") }}</label>
          <input v-model="form.author" class="input" placeholder="Your Name" />
        </section>

        <!-- Logo -->
        <section class="card">
          <h2>{{ t("themeSettings.logo") }}</h2>
          <p class="hint">{{ t("themeSettings.logoHint") }}</p>
          <div class="logo-row">
            <div class="logo-preview">
              <img v-if="logoUrl" :src="logoUrl" alt="logo" />
              <span v-else class="placeholder">No Logo</span>
            </div>
            <div class="logo-actions">
              <button class="btn" @click="handleUploadLogo">{{ t("themeSettings.uploadLogo") }}</button>
              <button v-if="form.logo" class="btn btn-danger" @click="handleRemoveLogo">
                {{ t("themeSettings.removeLogo") }}
              </button>
            </div>
          </div>
        </section>

        <!-- 页头/页尾 -->
        <section class="card">
          <h2>{{ t("themeSettings.headerFooter") }}</h2>
          <p class="hint">{{ t("themeSettings.headerFooterHint") }}</p>
          <label class="field-label">{{ t("themeSettings.headerHtml") }}</label>
          <textarea v-model="form.header_html" class="code-area" rows="4" placeholder='<!-- 统计代码 / 公告 / 广告 -->'></textarea>
          <label class="field-label">{{ t("themeSettings.footerHtml") }}</label>
          <textarea v-model="form.footer_html" class="code-area" rows="4" placeholder='<!-- 统计 / 备案 / 广告 -->'></textarea>
        </section>

        <!-- 广告位 -->
        <section class="card">
          <h2>{{ t("themeSettings.ads") }}</h2>
          <p class="hint">{{ t("themeSettings.adsHint") }}</p>
          <label class="field-label">{{ t("themeSettings.adHeader") }}</label>
          <textarea v-model="form.ads.header_html" class="code-area" rows="3"></textarea>
          <label class="field-label">{{ t("themeSettings.adSidebar") }}</label>
          <textarea v-model="form.ads.sidebar_html" class="code-area" rows="3"></textarea>
          <label class="field-label">{{ t("themeSettings.adPostFooter") }}</label>
          <textarea v-model="form.ads.post_footer_html" class="code-area" rows="3"></textarea>
          <label class="field-label">{{ t("themeSettings.adSiteFooter") }}</label>
          <textarea v-model="form.ads.site_footer_html" class="code-area" rows="3"></textarea>
        </section>

        <!-- 评论系统 -->
        <section class="card">
          <h2>{{ t("themeSettings.comments") }}</h2>
          <p class="hint">{{ t("themeSettings.commentsHint") }}</p>
          <label class="field-label">{{ t("themeSettings.commentSystem") }}</label>
          <select v-model="form.comments.system" class="select">
            <option value="none">{{ t("themeSettings.csNone") }}</option>
            <option value="disqus">Disqus</option>
            <option value="changyan">{{ t("themeSettings.csChangyan") }}</option>
            <option value="livere">LiveRe</option>
            <option value="valine">Valine</option>
            <option value="utterances">Utterances (GitHub Issues)</option>
            <option value="giscus">Giscus (GitHub Discussions)</option>
            <option value="custom">{{ t("themeSettings.csCustom") }}</option>
          </select>
          <div v-if="currentCommentFields.length" class="comment-fields">
            <div v-for="field in currentCommentFields" :key="field.key" class="form-row">
              <label class="field-label">{{ field.label }}</label>
              <input
                v-if="field.key !== 'html'"
                type="text"
                class="input"
                :placeholder="field.placeholder"
                :value="getParam(field.key)"
                @input="setParam(field.key, ($event.target as HTMLInputElement).value)"
              />
              <textarea
                v-else
                class="code-area"
                rows="5"
                :placeholder="field.placeholder"
                :value="getParam(field.key)"
                @input="setParam(field.key, ($event.target as HTMLTextAreaElement).value)"
              ></textarea>
            </div>
          </div>
          <p v-else class="hint">{{ t("themeSettings.csNoneHint") }}</p>
        </section>

        <!-- 备案 & 友链 -->
        <section class="card">
          <h2>{{ t("themeSettings.misc") }}</h2>
          <label class="field-label">{{ t("themeSettings.icp") }}</label>
          <input v-model="form.icp" type="text" class="input" placeholder="京ICP备12345678号" />
          <h3 class="sub-title">{{ t("themeSettings.friendLinks") }}</h3>
          <div v-for="(link, i) in form.links" :key="i" class="link-row">
            <input v-model="link.name" class="input" :placeholder="t('themeSettings.linkName')" />
            <input v-model="link.url" class="input" :placeholder="t('themeSettings.linkUrl')" />
            <input v-model="link.description" class="input" :placeholder="t('themeSettings.linkDesc')" />
            <button class="btn btn-danger btn-sm" @click="removeLink(i)">×</button>
          </div>
          <button class="btn btn-sm" @click="addLink">+ {{ t("themeSettings.addLink") }}</button>
        </section>

        <div class="bottom-save">
          <button class="btn-save" :disabled="saving" @click="handleSave">
            {{ saving ? t("themeSettings.saving") : t("themeSettings.save") }}
          </button>
          <span v-if="settingsMsg" class="msg">{{ settingsMsg }}</span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.themes-view {
  padding: 20px 24px;
  max-width: 1200px;
  margin: 0 auto;
}

/* Tab 栏 */
.tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 20px;
  border-bottom: 2px solid #e5e7eb;
}
.tab-btn {
  padding: 10px 24px;
  background: transparent;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -2px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  color: #6b7280;
  transition: 0.15s;
}
.tab-btn:hover {
  color: #2563eb;
}
.tab-btn.active {
  color: #2563eb;
  border-bottom-color: #2563eb;
}

.page-header {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  margin-bottom: 20px;
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
.active-banner .label { color: #6b7280; margin-right: 6px; }
.active-banner .author { color: #6b7280; margin-left: 8px; }

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
.card-cover img { width: 100%; height: 100%; object-fit: cover; }
.cover-placeholder { font-size: 48px; font-weight: 700; color: #9ca3af; }
.card-body { padding: 12px 16px; }
.card-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}
.card-head h3 { font-size: 16px; font-weight: 600; }
.active-badge {
  font-size: 11px;
  padding: 2px 8px;
  background: #2563eb;
  color: #fff;
  border-radius: 10px;
}
.author { font-size: 12px; color: #6b7280; margin-bottom: 8px; }
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

/* 设置 Tab 样式 */
.settings-tab { max-width: 820px; }
.btn-save {
  padding: 8px 20px;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-save:disabled { opacity: 0.5; cursor: not-allowed; }

.card {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 20px;
}
.card h2 { font-size: 16px; font-weight: 600; margin-bottom: 6px; }
.card h3.sub-title { font-size: 14px; font-weight: 600; margin: 20px 0 10px; }
.hint { font-size: 12px; color: #6b7280; margin-bottom: 14px; }
.field-label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: #374151;
  margin: 12px 0 4px;
}
.code-area, .input, .select {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  font-size: 13px;
  font-family: "Courier New", monospace;
  box-sizing: border-box;
}
.input, .select { font-family: inherit; }
.code-area { resize: vertical; }
.logo-row { display: flex; align-items: center; gap: 20px; }
.logo-preview {
  width: 120px;
  height: 120px;
  border: 1px dashed #d1d5db;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f9fafb;
  overflow: hidden;
}
.logo-preview img { max-width: 100%; max-height: 100%; object-fit: contain; }
.logo-preview .placeholder { font-size: 12px; color: #9ca3af; }
.logo-actions { display: flex; gap: 10px; }
.btn {
  padding: 6px 14px;
  background: #f3f4f6;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn:hover { background: #e5e7eb; }
.btn-danger { color: #dc2626; border-color: #fecaca; }
.btn-danger:hover { background: #fef2f2; }
.btn-sm { padding: 4px 10px; font-size: 12px; }
.comment-fields { margin-top: 8px; }
.link-row {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
  align-items: center;
}
.link-row .input { flex: 1; }
.bottom-save {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 20px 0;
}
.bottom-save .msg { font-size: 13px; color: #059669; }
</style>
