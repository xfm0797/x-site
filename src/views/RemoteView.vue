<script setup lang="ts">
/**
 * RemoteView — 远程部署目标管理
 *
 * 管理多个部署目标（local / git / netlify / vercel），
 * 持久化到 SiteConfig.deploy_targets。
 */
import { ref, reactive, onMounted } from "vue";
import {
  getThemeSettings,
  saveThemeSettings,
  type SiteConfig,
  type DeployTarget,
} from "../api/tauri";
import { useI18n } from "../i18n";

const { t } = useI18n();

const loading = ref(false);
const saving = ref(false);
const message = ref("");

// 完整 SiteConfig（保存时整体回写）
const config = reactive<SiteConfig>({
  title: "", description: "", author: "", url: "",
  theme: "default", posts_per_page: 10, links: [],
  logo: "", header_html: "", footer_html: "",
  ads: { header_html: "", sidebar_html: "", post_footer_html: "", site_footer_html: "" },
  comments: { system: "none", params: {} },
  icp: "",
  deploy_targets: [],
});

const targets = ref<DeployTarget[]>([]);

// 新增/编辑表单
const editing = ref(false);
const editIndex = ref(-1); // -1 表示新增
const form = reactive<DeployTarget>({
  name: "",
  config: { type: "local" },
});

const deployTypes = [
  { value: "local", label: "本地复制" },
  { value: "git", label: "Git 仓库" },
  { value: "netlify", label: "Netlify" },
  { value: "vercel", label: "Vercel" },
];

onMounted(async () => {
  loading.value = true;
  try {
    const cfg = await getThemeSettings();
    Object.assign(config, cfg);
    if (!config.deploy_targets) config.deploy_targets = [];
    targets.value = config.deploy_targets;
  } catch (e) {
    message.value = `加载失败：${e}`;
  } finally {
    loading.value = false;
  }
});

function startAdd() {
  editIndex.value = -1;
  form.name = "";
  form.config = { type: "local" };
  editing.value = true;
}

function startEdit(i: number) {
  editIndex.value = i;
  const tgt = targets.value[i];
  form.name = tgt.name;
  form.config = { ...tgt.config };
  editing.value = true;
}

function cancelEdit() {
  editing.value = false;
}

function saveTarget() {
  if (!form.name.trim()) {
    message.value = t("remote.nameRequired");
    return;
  }
  const tgt: DeployTarget = {
    name: form.name.trim(),
    config: { ...form.config },
  };
  if (editIndex.value >= 0) {
    targets.value[editIndex.value] = tgt;
  } else {
    targets.value.push(tgt);
  }
  config.deploy_targets = targets.value;
  editing.value = false;
  // 自动保存
  void saveTargets();
}

function removeTarget(i: number) {
  targets.value.splice(i, 1);
  config.deploy_targets = targets.value;
  void saveTargets();
}

async function saveTargets() {
  saving.value = true;
  message.value = "";
  try {
    await saveThemeSettings(config);
    message.value = t("remote.saved");
  } catch (e) {
    message.value = `${t("common.error")}: ${e}`;
  } finally {
    saving.value = false;
  }
}

function typeLabel(type: string): string {
  const found = deployTypes.find((d) => d.value === type);
  return found ? found.label : type;
}
</script>

<template>
  <div class="remote-view">
    <header class="page-header">
      <h1>{{ t("remote.title") }}</h1>
      <button class="btn-primary" @click="startAdd" v-if="!editing">
        {{ t("remote.addTarget") }}
      </button>
    </header>

    <div v-if="loading" class="loading">{{ t("home.loading") }}</div>
    <template v-else>
      <!-- 目标列表 -->
      <div v-if="targets.length === 0 && !editing" class="empty">
        {{ t("remote.empty") }}
      </div>

      <div v-for="(tgt, i) in targets" :key="i" class="target-card" v-if="!editing">
        <div class="target-info">
          <span class="target-name">{{ tgt.name }}</span>
          <span class="target-type" :class="'badge-' + tgt.config.type">
            {{ typeLabel(tgt.config.type) }}
          </span>
        </div>
        <div class="target-detail">
          <span v-if="tgt.config.type === 'local'">{{ tgt.config.target }}</span>
          <span v-else-if="tgt.config.type === 'git'">{{ tgt.config.repo }} ({{ tgt.config.branch }})</span>
          <span v-else-if="tgt.config.type === 'netlify'">site: {{ tgt.config.site_id }}</span>
          <span v-else-if="tgt.config.type === 'vercel'">project: {{ tgt.config.project_id }}</span>
        </div>
        <div class="target-actions">
          <button class="btn btn-sm" @click="startEdit(i)">{{ t("remote.edit") }}</button>
          <button class="btn btn-sm btn-danger" @click="removeTarget(i)">{{ t("remote.delete") }}</button>
        </div>
      </div>

      <!-- 新增/编辑表单 -->
      <div v-if="editing" class="edit-form card">
        <h2>{{ editIndex >= 0 ? t("remote.editTarget") : t("remote.addTarget") }}</h2>

        <label class="field-label">{{ t("remote.name") }}</label>
        <input v-model="form.name" class="input" :placeholder="t('remote.namePlaceholder')" />

        <label class="field-label">{{ t("remote.type") }}</label>
        <select v-model="form.config.type" class="select">
          <option v-for="d in deployTypes" :key="d.value" :value="d.value">{{ d.label }}</option>
        </select>

        <!-- local -->
        <template v-if="form.config.type === 'local'">
          <label class="field-label">{{ t("remote.target") }}</label>
          <input v-model="form.config.target" class="input" placeholder="/path/to/deploy" />
        </template>

        <!-- git -->
        <template v-if="form.config.type === 'git'">
          <label class="field-label">{{ t("remote.repo") }}</label>
          <input v-model="form.config.repo" class="input" placeholder="https://github.com/user/repo.git" />
          <label class="field-label">{{ t("remote.branch") }}</label>
          <input v-model="form.config.branch" class="input" placeholder="gh-pages" />
          <label class="field-label">{{ t("remote.message") }}</label>
          <input v-model="form.config.message" class="input" placeholder="Deploy site" />
        </template>

        <!-- netlify -->
        <template v-if="form.config.type === 'netlify'">
          <label class="field-label">{{ t("remote.token") }}</label>
          <input v-model="form.config.token" class="input" type="password" placeholder="nfp_..." />
          <label class="field-label">{{ t("remote.siteId") }}</label>
          <input v-model="form.config.site_id" class="input" placeholder="abc-123-def" />
        </template>

        <!-- vercel -->
        <template v-if="form.config.type === 'vercel'">
          <label class="field-label">{{ t("remote.token") }}</label>
          <input v-model="form.config.token" class="input" type="password" placeholder="vercel_..." />
          <label class="field-label">{{ t("remote.projectId") }}</label>
          <input v-model="form.config.project_id" class="input" placeholder="prj_..." />
          <label class="field-label">{{ t("remote.teamId") }} ({{ t("remote.optional") }})</label>
          <input v-model="form.config.team_id" class="input" placeholder="team_..." />
        </template>

        <div class="form-actions">
          <button class="btn-primary" @click="saveTarget">{{ t("remote.save") }}</button>
          <button class="btn" @click="cancelEdit">{{ t("remote.cancel") }}</button>
        </div>
      </div>

      <p v-if="message" class="msg">{{ message }}</p>
    </template>
  </div>
</template>

<style scoped>
.remote-view {
  padding: 20px 24px;
  max-width: 820px;
  margin: 0 auto;
}
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}
.page-header h1 { font-size: 22px; font-weight: 700; }
.loading { padding: 60px; text-align: center; color: #6b7280; }
.empty { padding: 40px; text-align: center; color: #9ca3af; font-size: 14px; }

.target-card {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 16px 20px;
  margin-bottom: 12px;
  display: flex;
  align-items: center;
  gap: 16px;
}
.target-info { flex: 1; display: flex; align-items: center; gap: 10px; }
.target-name { font-size: 15px; font-weight: 600; }
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
.target-detail {
  flex: 2;
  font-size: 12px;
  color: #6b7280;
  font-family: "JetBrains Mono", Consolas, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.target-actions { display: flex; gap: 8px; }

.card {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 20px;
}
.card h2 { font-size: 16px; font-weight: 600; margin-bottom: 16px; }
.field-label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: #374151;
  margin: 12px 0 4px;
}
.input, .select {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  font-size: 13px;
  box-sizing: border-box;
}
.form-actions { display: flex; gap: 10px; margin-top: 20px; }
.btn-primary {
  padding: 8px 20px;
  background: #2563eb;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn {
  padding: 8px 16px;
  background: #f3f4f6;
  border: 1px solid #d1d5db;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.btn-sm { padding: 4px 10px; font-size: 12px; }
.btn-danger { color: #dc2626; border-color: #fecaca; }
.btn-danger:hover { background: #fef2f2; }
.msg { margin-top: 16px; padding: 10px; background: #f0f0f0; border-radius: 4px; font-size: 13px; }
</style>
