<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { usePostStore } from "../stores/post";
import { useI18n } from "../i18n";

const router = useRouter();
const store = usePostStore();
const { t } = useI18n();

const keyword = ref("");
const statusFilter = ref<"all" | "published" | "draft">("all");
const sortBy = ref<"updated" | "created" | "title">("updated");

/** 搜索（标题/slug/分类/标签）+ 状态筛选 + 排序 */
const visiblePosts = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  const list = store.posts.filter((post) => {
    if (statusFilter.value !== "all" && post.status !== statusFilter.value) {
      return false;
    }
    if (!kw) return true;
    return [post.title, post.slug, post.category, ...post.tags].some((v) =>
      (v || "").toLowerCase().includes(kw),
    );
  });
  return [...list].sort((a, b) => {
    if (sortBy.value === "title") {
      return (a.title || "").localeCompare(b.title || "");
    }
    if (sortBy.value === "created") {
      return b.created_at.localeCompare(a.created_at);
    }
    return b.updated_at.localeCompare(a.updated_at);
  });
});

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

    <div v-else-if="store.posts.length" class="filters">
      <input
        v-model="keyword"
        class="search"
        type="search"
        :placeholder="t('home.searchPlaceholder')"
      />
      <select v-model="statusFilter">
        <option value="all">{{ t("home.filterAll") }}</option>
        <option value="published">{{ t("home.published") }}</option>
        <option value="draft">{{ t("home.draft") }}</option>
      </select>
      <select v-model="sortBy">
        <option value="updated">{{ t("home.sortUpdated") }}</option>
        <option value="created">{{ t("home.sortCreated") }}</option>
        <option value="title">{{ t("home.sortTitle") }}</option>
      </select>
    </div>

    <ul v-if="!store.loading && visiblePosts.length" class="post-list">
      <li
        v-for="post in visiblePosts"
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

    <p v-else-if="!store.loading && store.posts.length" class="empty">
      {{ t("home.noMatch") }}
    </p>

    <p v-else-if="!store.loading" class="empty">{{ t("home.empty") }}</p>
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
.filters {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.filters .search {
  flex: 1;
  padding: 6px 10px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
}
.filters select {
  padding: 6px 8px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 14px;
  background: #fff;
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
