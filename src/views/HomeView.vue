<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { usePostStore } from "../stores/post";
import { useI18n } from "../i18n";

const router = useRouter();
const store = usePostStore();
const { t } = useI18n();

onMounted(() => {
  store.fetchPosts();
});

/** 删除文章（需确认），失败时显式提示 */
async function onDelete(post: { id: string; title: string }) {
  const title = post.title || t("home.untitled");
  if (!window.confirm(t("home.deleteConfirm", { title }))) return;
  try {
    await store.removePost(post.id);
  } catch (e: any) {
    window.alert(t("common.error", { msg: String(e?.message ?? e) }));
  }
}
</script>

<template>
  <div class="home">
    <div class="toolbar">
      <h1>{{ t("home.title") }}</h1>
      <button @click="router.push('/editor')">{{ t("home.newPost") }}</button>
    </div>

    <p v-if="store.listError" class="list-error">
      {{ t("common.error", { msg: store.listError }) }}
    </p>

    <p v-if="store.loading">{{ t("home.loading") }}</p>

    <ul v-else-if="store.posts.length" class="post-list">
      <li
        v-for="post in store.posts"
        :key="post.id"
        @click="router.push(`/editor/${post.id}`)"
      >
        <span class="title">{{ post.title || t("home.untitled") }}</span>
        <span class="status" :class="post.status">
          {{ post.status === "published" ? t("home.published") : t("home.draft") }}
        </span>
        <span class="date">{{ post.updated_at.slice(0, 10) }}</span>
        <button class="delete-btn" :title="t('home.delete')" @click.stop="onDelete(post)">✕</button>
      </li>
    </ul>

    <p v-else class="empty">{{ t("home.empty") }}</p>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}
.toolbar h1 {
  font-size: 24px;
}
.toolbar button {
  padding: 8px 16px;
  background: #4a9eff;
  color: #fff;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
.post-list {
  list-style: none;
  padding: 0;
}
.post-list li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border-bottom: 1px solid #eee;
  cursor: pointer;
}
.post-list li:hover {
  background: #f5f5f5;
}
.title {
  flex: 1;
}
.status {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  background: #e0e0e0;
}
.status.published {
  background: #d4edda;
  color: #155724;
}
.status.draft {
  background: #fff3cd;
  color: #856404;
}
.date {
  font-size: 12px;
  color: #999;
}
.delete-btn {
  padding: 2px 8px;
  font-size: 12px;
  color: #dc2626;
  background: transparent;
  border: 1px solid #fecaca;
  border-radius: 4px;
  cursor: pointer;
}
.delete-btn:hover {
  background: #fef2f2;
}
.list-error {
  color: #b91c1c;
}
.empty {
  color: #999;
  text-align: center;
  margin-top: 60px;
}
</style>
