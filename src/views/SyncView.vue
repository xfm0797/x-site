<script setup lang="ts">
/**
 * SyncView — 同步：生成站点 + 依次部署全部远程目标
 *
 * 流程：generateSite → 遍历 deploy_targets → 逐个 deploy
 */
import { ref, onMounted } from "vue";
import {
  getThemeSettings,
  generateSite,
  deploy,
  type DeployTarget,
} from "../api/tauri";
import { useI18n } from "../i18n";

const { t } = useI18n();

const outputDir = ref("");
const targets = ref<DeployTarget[]>([]);
const loading = ref(true);
const syncing = ref(false);
const message = ref("");

// 同步结果：每个目标的部署状态
interface SyncResult {
  name: string;
  type: string;
  status: "pending" | "deploying" | "success" | "failed";
  result: string;
}
const results = ref<SyncResult[]>([]);
const generatePercent = ref(0);

onMounted(async () => {
  try {
    const cfg = await getThemeSettings();
    targets.value = cfg.deploy_targets || [];
  } catch (e) {
    message.value = `加载失败：${e}`;
  } finally {
    loading.value = false;
  }
});

async function startSync() {
  if (!outputDir.value) {
    message.value = t("sync.errorEmpty");
    return;
  }
  if (targets.value.length === 0) {
    message.value = t("sync.noTargets");
    return;
  }

  syncing.value = true;
  message.value = "";
  generatePercent.value = 0;
  results.value = targets.value.map((tgt) => ({
    name: tgt.name,
    type: tgt.config.type,
    status: "pending",
    result: "",
  }));

  try {
    // 1. 生成站点
    message.value = t("sync.generating");
    await generateSite(outputDir.value);
    generatePercent.value = 100;

    // 2. 依次部署每个目标
    for (let i = 0; i < targets.value.length; i++) {
      const tgt = targets.value[i];
      results.value[i].status = "deploying";
      message.value = t("sync.deploying", { name: tgt.name, index: i + 1, total: targets.value.length });
      try {
        const res = await deploy(outputDir.value, tgt.config);
        results.value[i].status = "success";
        results.value[i].result = res;
      } catch (e) {
        results.value[i].status = "failed";
        results.value[i].result = String(e);
      }
    }

    const ok = results.value.filter((r) => r.status === "success").length;
    const fail = results.value.filter((r) => r.status === "failed").length;
    message.value = t("sync.done", { ok, fail, total: results.value.length });
  } catch (e) {
    message.value = t("sync.generateFailed", { err: String(e) });
  } finally {
    syncing.value = false;
  }
}
</script>

<template>
  <div class="sync-view">
    <h1>{{ t("sync.title") }}</h1>
    <p class="hint">{{ t("sync.hint") }}</p>

    <div v-if="loading" class="loading">{{ t("home.loading") }}</div>
    <template v-else>
      <!-- 输出目录 -->
      <section class="card">
        <label>{{ t("sync.outputDir") }}</label>
        <input v-model="outputDir" :placeholder="t('preview.outputPlaceholder')" />
      </section>

      <!-- 目标概览 -->
      <section class="card">
        <h2>{{ t("sync.targets") }} ({{ targets.length }})</h2>
        <div v-if="targets.length === 0" class="empty">{{ t("sync.noTargets") }}</div>
        <div v-for="(tgt, i) in targets" :key="i" class="target-item">
          <span class="target-name">{{ tgt.name }}</span>
          <span class="target-type" :class="'badge-' + tgt.config.type">{{ tgt.config.type }}</span>
          <span class="target-status" :class="'status-' + (results[i]?.status || 'pending')">
            {{ results[i] ? t("sync." + results[i].status) : "" }}
          </span>
          <div v-if="results[i]?.result" class="target-result">{{ results[i].result }}</div>
        </div>
      </section>

      <!-- 生成进度 -->
      <div v-if="syncing && generatePercent < 100" class="progress-wrap">
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: generatePercent + '%' }"></div>
        </div>
      </div>

      <!-- 同步按钮 -->
      <button
        class="btn-primary"
        :disabled="syncing || targets.length === 0"
        @click="startSync"
      >
        {{ syncing ? t("sync.syncing") : t("sync.start") }}
      </button>

      <p v-if="message" class="msg">{{ message }}</p>
    </template>
  </div>
</template>

<style scoped>
.sync-view { max-width: 820px; }
h1 { font-size: 22px; font-weight: 700; margin-bottom: 8px; }
.hint { font-size: 13px; color: #6b7280; margin-bottom: 20px; }
.loading { padding: 60px; text-align: center; color: #6b7280; }

.card { background: #fff; border: 1px solid #e5e7eb; border-radius: 8px; padding: 20px; margin-bottom: 16px; }
.card h2 { font-size: 16px; font-weight: 600; margin-bottom: 12px; }
label { display: block; font-size: 13px; color: #666; margin: 8px 0 4px; }
input { width: 100%; padding: 8px; border: 1px solid #ddd; border-radius: 4px; box-sizing: border-box; }

.empty { padding: 20px; text-align: center; color: #9ca3af; font-size: 13px; }
.target-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 0;
  border-bottom: 1px solid #f3f4f6;
  flex-wrap: wrap;
}
.target-name { font-size: 14px; font-weight: 500; flex: 1; }
.target-type {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  color: #fff;
}
.badge-local { background: #6b7280; }
.badge-git { background: #2563eb; }
.badge-netlify { background: #16a34a; }
.badge-vercel { background: #000; }
.target-status { font-size: 12px; font-weight: 500; }
.status-pending { color: #9ca3af; }
.status-deploying { color: #d97706; }
.status-success { color: #16a34a; }
.status-failed { color: #dc2626; }
.target-result {
  width: 100%;
  font-size: 11px;
  color: #6b7280;
  font-family: "JetBrains Mono", Consolas, monospace;
  margin-top: 4px;
  word-break: break-all;
}

.progress-wrap { margin: 16px 0; }
.progress-bar { width: 100%; height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden; }
.progress-fill { height: 100%; background: linear-gradient(90deg, #4a9eff, #2563eb); transition: width 0.3s ease; }

.btn-primary { padding: 10px 24px; background: #4a9eff; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 14px; }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.msg { margin-top: 16px; padding: 10px; background: #f0f0f0; border-radius: 4px; font-size: 13px; }
</style>
