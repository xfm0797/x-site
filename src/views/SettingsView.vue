<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import {
  generateSite,
  deploy,
  startPreview,
  stopPreview,
  getPreviewPort,
  type DeployConfig,
} from "../api/tauri";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { useI18n } from "../i18n";

const { t } = useI18n();

// 生成状态
const outputDir = ref("");
const generating = ref(false);
const progress = ref({ current: 0, total: 0, percent: 0, file: "" });
const message = ref("");

// 部署状态
const deployType = ref<"local" | "git">("local");
const deployTarget = ref("");
const gitRepo = ref("");
const gitBranch = ref("gh-pages");
const deploying = ref(false);

// 预览状态
const previewPort = ref(0);
const previewStarting = ref(false);
const previewUrl = ref("");

// Tauri 事件订阅句柄
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
  // 恢复预览状态（应用重启后）
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
    message.value = t("settings.generate.errorEmpty");
    return;
  }
  generating.value = true;
  message.value = "";
  progress.value = { current: 0, total: 0, percent: 0, file: "" };
  try {
    await generateSite(outputDir.value);
    message.value = t("settings.generate.done");
  } catch (e) {
    message.value = t("common.error", { msg: String(e) });
  } finally {
    generating.value = false;
  }
}

async function doDeploy() {
  if (!outputDir.value) {
    message.value = t("settings.generate.errorEmpty");
    return;
  }
  deploying.value = true;
  message.value = "";
  try {
    const config: DeployConfig =
      deployType.value === "git"
        ? {
            type: "git",
            repo: gitRepo.value,
            branch: gitBranch.value,
          }
        : { type: "local", target: deployTarget.value };
    await deploy(outputDir.value, config);
    message.value = t("settings.deploy.done");
  } catch (e) {
    message.value = t("common.error", { msg: String(e) });
  } finally {
    deploying.value = false;
  }
}

// ============================================================
// 本地预览控制
// ============================================================

async function startLocalPreview() {
  if (!outputDir.value) {
    message.value = t("settings.preview.errorEmpty");
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
    message.value = t("settings.preview.started", { url: previewUrl.value });
  } catch (e) {
    message.value = t("settings.preview.startFail", { err: String(e) });
  } finally {
    previewStarting.value = false;
  }
}

async function stopLocalPreview() {
  try {
    await stopPreview();
    previewPort.value = 0;
    previewUrl.value = "";
    message.value = t("settings.preview.stopped");
  } catch (e) {
    message.value = t("settings.preview.stopFail", { err: String(e) });
  }
}

/** 在系统默认浏览器打开预览 */
async function openInBrowser() {
  if (!previewUrl.value) return;
  try {
    await open(previewUrl.value);
  } catch {
    // 浏览器环境降级
    window.open(previewUrl.value, "_blank");
  }
}

/** 刷新 iframe（重新加载 src） */
function refreshIframe() {
  const iframe = document.querySelector<HTMLIFrameElement>(".preview-iframe");
  if (iframe) {
    // 重新加载：用 dummy query 强制刷新
    const src = iframe.src.split("?")[0];
    iframe.src = `${src}?_t=${Date.now()}`;
  }
}

</script>

<template>
  <div class="settings">
    <h1>{{ t("settings.title") }}</h1>

    <!-- ============ 生成站点 ============ -->
    <section>
      <h2>{{ t("settings.generate.title") }}</h2>
      <label>{{ t("settings.generate.outputDir") }}</label>
      <input v-model="outputDir" :placeholder="t('settings.generate.outputPlaceholder')" />

      <button class="btn-primary" :disabled="generating" @click="generate">
        {{ generating ? t("settings.generate.generating") : t("settings.generate.btn") }}
      </button>

      <!-- 进度条 -->
      <div v-if="generating || progress.percent > 0" class="progress-wrap">
        <div class="progress-bar">
          <div
            class="progress-fill"
            :style="{ width: progress.percent + '%' }"
          ></div>
        </div>
        <div class="progress-info">
          <span>{{ progress.percent }}%</span>
          <span class="progress-file" v-if="progress.file">
            {{ progress.current }}/{{ progress.total }} — {{ progress.file }}
          </span>
        </div>
      </div>
    </section>

    <!-- ============ 部署 ============ -->
    <section>
      <h2>{{ t("settings.deploy.title") }}</h2>
      <label>{{ t("settings.deploy.type") }}</label>
      <select v-model="deployType">
        <option value="local">{{ t("settings.deploy.local") }}</option>
        <option value="git">{{ t("settings.deploy.git") }}</option>
      </select>

      <div v-if="deployType === 'local'">
        <label>{{ t("settings.deploy.targetPath") }}</label>
        <input v-model="deployTarget" :placeholder="t('settings.deploy.targetPlaceholder')" />
      </div>
      <div v-else>
        <label>{{ t("settings.deploy.gitRepo") }}</label>
        <input
          v-model="gitRepo"
          :placeholder="t('settings.deploy.repoPlaceholder')"
        />
        <label>{{ t("settings.deploy.branch") }}</label>
        <input v-model="gitBranch" placeholder="gh-pages" />
      </div>

      <button class="btn-primary" :disabled="deploying" @click="doDeploy">
        {{ deploying ? t("settings.deploy.deploying") : t("settings.deploy.btn") }}
      </button>
    </section>

    <!-- ============ 本地预览 ============ -->
    <section>
      <h2>{{ t("settings.preview.title") }}</h2>
      <div class="preview-actions">
        <button
          class="btn-primary"
          :disabled="previewStarting || !outputDir"
          @click="startLocalPreview"
        >
          {{ previewPort > 0 ? t("settings.preview.restart") : t("settings.preview.start") }}
        </button>
        <button
          v-if="previewPort > 0"
          class="btn-secondary"
          @click="stopLocalPreview"
        >
          {{ t("settings.preview.stop") }}
        </button>
        <button
          v-if="previewPort > 0"
          class="btn-secondary"
          @click="openInBrowser"
        >
          {{ t("settings.preview.openBrowser") }}
        </button>
      </div>

      <div v-if="previewPort > 0" class="preview-info">
        <a :href="previewUrl" target="_blank">{{ previewUrl }}</a>
      </div>

      <!-- iframe 内嵌预览 -->
      <div v-if="previewPort > 0" class="preview-frame-wrap">
        <div class="preview-toolbar">
          <button class="btn-mini" @click="refreshIframe">{{ t("settings.preview.refresh") }}</button>
          <span class="preview-status">{{ t("settings.preview.running") }}</span>
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
.settings {
  max-width: 800px;
}
section {
  margin-bottom: 28px;
}
h2 {
  margin-bottom: 12px;
}
label {
  display: block;
  font-size: 13px;
  color: #666;
  margin: 8px 0 4px;
}
input,
select {
  width: 100%;
  padding: 8px;
  border: 1px solid #ddd;
  border-radius: 4px;
  margin-bottom: 8px;
}
.btn-primary {
  padding: 8px 20px;
  background: #4a9eff;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 进度条 */
.progress-wrap {
  margin-top: 14px;
}
.progress-bar {
  width: 100%;
  height: 8px;
  background: #e5e7eb;
  border-radius: 4px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #4a9eff, #2563eb);
  transition: width 0.3s ease;
}
.progress-info {
  display: flex;
  justify-content: space-between;
  margin-top: 6px;
  font-size: 12px;
  color: #6b7280;
}
.progress-file {
  color: #9ca3af;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 70%;
  font-family: "JetBrains Mono", Consolas, monospace;
}

.msg {
  margin-top: 16px;
  padding: 10px;
  background: #f0f0f0;
  border-radius: 4px;
}

/* 预览 */
.preview-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.btn-secondary {
  padding: 8px 16px;
  background: #fff;
  color: #374151;
  border: 1px solid #ddd;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-secondary:hover {
  background: #f3f4f6;
}
.preview-info {
  margin-bottom: 12px;
  font-size: 13px;
}
.preview-info a {
  color: #2563eb;
  font-family: "JetBrains Mono", Consolas, monospace;
}
.preview-frame-wrap {
  border: 1px solid #e5e7eb;
  border-radius: 6px;
  overflow: hidden;
}
.preview-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  background: #f9fafb;
  border-bottom: 1px solid #e5e7eb;
}
.preview-status {
  font-size: 12px;
  color: #16a34a;
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
.preview-iframe {
  width: 100%;
  height: 480px;
  border: none;
  background: #fff;
}
</style>
