<script setup lang="ts">
/**
 * PreviewView — 生成站点 + 本地预览
 *
 * 从原 SettingsView 移出：output_dir 输入 + 生成 + 进度 + iframe 预览
 */
import { ref, onMounted, onBeforeUnmount } from "vue";
import {
  generateSite,
  startPreview,
  stopPreview,
  getPreviewPort,
} from "../api/tauri";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { useI18n } from "../i18n";

const { t } = useI18n();

const outputDir = ref("");
const generating = ref(false);
const progress = ref({ current: 0, total: 0, percent: 0, file: "" });
const message = ref("");

const previewPort = ref(0);
const previewStarting = ref(false);
const previewUrl = ref("");

let unlistenProgress: UnlistenFn | null = null;

onMounted(async () => {
  unlistenProgress = await listen<{
    current: number;
    total: number;
    percent: number;
    file: string;
  }>("generate://progress", (e) => {
    progress.value = e.payload;
  });
  const port = await getPreviewPort();
  if (port > 0) {
    previewPort.value = port;
    previewUrl.value = `http://127.0.0.1:${port}`;
  }
});

onBeforeUnmount(() => {
  unlistenProgress?.();
});

async function generate() {
  if (!outputDir.value) {
    message.value = t("preview.errorEmpty");
    return;
  }
  generating.value = true;
  message.value = "";
  progress.value = { current: 0, total: 0, percent: 0, file: "" };
  try {
    await generateSite(outputDir.value);
    message.value = t("preview.generateDone");
  } catch (e) {
    message.value = t("common.error", { msg: String(e) });
  } finally {
    generating.value = false;
  }
}

async function startLocalPreview() {
  if (!outputDir.value) {
    message.value = t("preview.errorEmpty");
    return;
  }
  previewStarting.value = true;
  message.value = "";
  try {
    if (previewPort.value > 0) {
      await stopPreview();
    }
    const port = await startPreview(outputDir.value);
    previewPort.value = port;
    previewUrl.value = `http://127.0.0.1:${port}`;
    message.value = t("preview.started", { url: previewUrl.value });
  } catch (e) {
    message.value = t("preview.startFail", { err: String(e) });
  } finally {
    previewStarting.value = false;
  }
}

async function stopLocalPreview() {
  try {
    await stopPreview();
    previewPort.value = 0;
    previewUrl.value = "";
    message.value = t("preview.stopped");
  } catch (e) {
    message.value = t("preview.stopFail", { err: String(e) });
  }
}

async function openInBrowser() {
  if (!previewUrl.value) return;
  try {
    await open(previewUrl.value);
  } catch {
    window.open(previewUrl.value, "_blank");
  }
}

function refreshIframe() {
  const iframe = document.querySelector<HTMLIFrameElement>(".preview-iframe");
  if (iframe) {
    const src = iframe.src.split("?")[0];
    iframe.src = `${src}?_t=${Date.now()}`;
  }
}
</script>

<template>
  <div class="preview-view">
    <h1>{{ t("preview.title") }}</h1>

    <!-- 生成站点 -->
    <section>
      <h2>{{ t("preview.generate") }}</h2>
      <label>{{ t("preview.outputDir") }}</label>
      <input v-model="outputDir" :placeholder="t('preview.outputPlaceholder')" />

      <button class="btn-primary" :disabled="generating" @click="generate">
        {{ generating ? t("preview.generating") : t("preview.generateBtn") }}
      </button>

      <div v-if="generating || progress.percent > 0" class="progress-wrap">
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: progress.percent + '%' }"></div>
        </div>
        <div class="progress-info">
          <span>{{ progress.percent }}%</span>
          <span class="progress-file" v-if="progress.file">
            {{ progress.current }}/{{ progress.total }} — {{ progress.file }}
          </span>
        </div>
      </div>
    </section>

    <!-- 本地预览 -->
    <section>
      <h2>{{ t("preview.localPreview") }}</h2>
      <div class="preview-actions">
        <button
          class="btn-primary"
          :disabled="previewStarting || !outputDir"
          @click="startLocalPreview"
        >
          {{ previewPort > 0 ? t("preview.restart") : t("preview.start") }}
        </button>
        <button v-if="previewPort > 0" class="btn-secondary" @click="stopLocalPreview">
          {{ t("preview.stop") }}
        </button>
        <button v-if="previewPort > 0" class="btn-secondary" @click="openInBrowser">
          {{ t("preview.openBrowser") }}
        </button>
      </div>

      <div v-if="previewPort > 0" class="preview-info">
        <a :href="previewUrl" target="_blank">{{ previewUrl }}</a>
      </div>

      <div v-if="previewPort > 0" class="preview-frame-wrap">
        <div class="preview-toolbar">
          <button class="btn-mini" @click="refreshIframe">{{ t("preview.refresh") }}</button>
          <span class="preview-status">{{ t("preview.running") }}</span>
        </div>
        <iframe
          class="preview-iframe"
          :src="previewUrl"
          sandbox="allow-same-origin allow-scripts allow-forms"
        ></iframe>
      </div>
    </section>

    <p v-if="message" class="msg">{{ message }}</p>
  </div>
</template>

<style scoped>
.preview-view { max-width: 900px; }
section { margin-bottom: 28px; background: #fff; border: 1px solid #e5e7eb; border-radius: 8px; padding: 20px; }
h1 { font-size: 22px; font-weight: 700; margin-bottom: 20px; }
h2 { margin-bottom: 12px; font-size: 16px; }
label { display: block; font-size: 13px; color: #666; margin: 8px 0 4px; }
input { width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 8px; box-sizing: border-box; }
.btn-primary { padding: 8px 20px; background: #4a9eff; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

.progress-wrap { margin-top: 14px; }
.progress-bar { width: 100%; height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden; }
.progress-fill { height: 100%; background: linear-gradient(90deg, #4a9eff, #2563eb); transition: width 0.3s ease; }
.progress-info { display: flex; justify-content: space-between; margin-top: 6px; font-size: 12px; color: #6b7280; }
.progress-file { color: #9ca3af; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 70%; font-family: "JetBrains Mono", Consolas, monospace; }

.preview-actions { display: flex; gap: 8px; margin-bottom: 12px; }
.btn-secondary { padding: 8px 16px; background: #fff; color: #374151; border: 1px solid #ddd; border-radius: 4px; cursor: pointer; font-size: 13px; }
.btn-secondary:hover { background: #f3f4f6; }
.preview-info { margin-bottom: 12px; font-size: 13px; }
.preview-info a { color: #2563eb; font-family: "JetBrains Mono", Consolas, monospace; }
.preview-frame-wrap { border: 1px solid #e5e7eb; border-radius: 6px; overflow: hidden; }
.preview-toolbar { display: flex; align-items: center; justify-content: space-between; padding: 6px 12px; background: #f9fafb; border-bottom: 1px solid #e5e7eb; }
.preview-status { font-size: 12px; color: #16a34a; }
.btn-mini { padding: 4px 10px; font-size: 12px; border: 1px solid #e5e7eb; background: #fff; border-radius: 3px; cursor: pointer; color: #374151; }
.preview-iframe { width: 100%; height: 480px; border: none; background: #fff; }

.msg { margin-top: 16px; padding: 10px; background: #f0f0f0; border-radius: 4px; font-size: 13px; }
</style>
