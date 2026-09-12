<script setup lang="ts">
/**
 * TagsView — 标签管理视图
 *
 * 功能：
 * - 从 loadPosts 聚合所有标签，统计每个标签的文章数
 * - 标签云：按文章数计算字号，点击展开该标签下的文章列表
 * - 顶部显示标签总数、文章总数
 *
 * 数据来源：与文章列表同源（load_posts 命令），无额外 Rust 端命令。
 */
import { ref, computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import { usePostStore } from "../stores/post";
import { useI18n } from "../i18n";
import type { PostMeta } from "../api/tauri";

const router = useRouter();
const store = usePostStore();
const { t } = useI18n();

/** 当前展开的标签名（点击切换），null 表示不展开任何标签 */
const activeTag = ref<string | null>(null);

onMounted(() => {
  if (store.posts.length === 0) {
    void store.fetchPosts();
  }
});

/** 聚合：tag → 文章列表 */
const tagMap = computed<Map<string, PostMeta[]>>(() => {
  const m = new Map<string, PostMeta[]>();
  for (const p of store.posts) {
    for (const t of p.tags) {
      if (!m.has(t)) m.set(t, []);
      m.get(t)!.push(p);
    }
  }
  return m;
});

/** 标签列表（按文章数降序） */
const tags = computed(() => {
  return Array.from(tagMap.value.entries())
    .map(([name, posts]) => ({ name, posts, count: posts.length }))
    .sort((a, b) => b.count - a.count);
});

const maxCount = computed(() =>
  tags.value.length ? Math.max(...tags.value.map((t) => t.count)) : 1,
);

/** 根据文章数计算字号（14-28px 之间） */
function fontSize(count: number): string {
  if (maxCount.value === 0) return "14px";
  const ratio = count / maxCount.value;
  const size = 14 + ratio * 14;
  return `${size.toFixed(0)}px`;
}

/** 当前展开标签下的文章 */
const activeTagPosts = computed<PostMeta[]>(() => {
  if (!activeTag.value) return [];
  return tagMap.value.get(activeTag.value) ?? [];
});

function toggleTag(name: string) {
  activeTag.value = activeTag.value === name ? null : name;
}
</script>

<template>
  <div class="tags-view">
    <div class="page-header">
      <h1>{{ t("tags.title") }}</h1>
      <span class="counter" v-if="tags.length">
        {{ t("tags.counter", { tagCount: tags.length, postCount: store.posts.length }) }}
      </span>
    </div>

    <p v-if="store.loading">{{ t("home.loading") }}</p>

    <!-- 标签云 -->
    <div v-else-if="tags.length" class="tag-cloud">
      <button
        v-for="tag in tags"
        :key="tag.name"
        class="tag-chip"
        :class="{ active: activeTag === tag.name }"
        :style="{ fontSize: fontSize(tag.count) }"
        @click="toggleTag(tag.name)"
      >
        <span class="tag-name">#{{ tag.name }}</span>
        <span class="tag-count">{{ tag.count }}</span>
      </button>
    </div>
    <p v-else class="empty">{{ t("tags.empty") }}</p>

    <!-- 展开的文章列表 -->
    <section v-if="activeTag" class="tag-posts">
      <div class="section-header">
        <h2>
          {{ t("tags.sectionTitle", { tag: activeTag }) }}
          <span class="muted">({{ activeTagPosts.length }})</span>
        </h2>
        <button class="btn-mini" @click="activeTag = null">{{ t("tags.collapse") }}</button>
      </div>
      <ul class="post-list">
        <li
          v-for="post in activeTagPosts"
          :key="post.id"
          @click="router.push(`/editor/${post.id}`)"
        >
          <span class="title">{{ post.title || "(未命名)" }}</span>
          <span class="status" :class="post.status">
            {{ post.status === "published" ? t("home.published") : t("home.draft") }}
          </span>
          <time>{{ post.updated_at.slice(0, 10) }}</time>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.tags-view {
  max-width: 800px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 20px;
}
.page-header h1 {
  font-size: 24px;
  font-weight: 700;
}
.counter {
  font-size: 13px;
  color: #6b7280;
}

.tag-cloud {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 16px;
  padding: 20px 0;
}
.tag-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  background: #f3f4f6;
  border: 1px solid #e5e7eb;
  border-radius: 14px;
  cursor: pointer;
  color: #374151;
  line-height: 1.6;
  transition: all 0.15s;
}
.tag-chip:hover {
  background: #eff6ff;
  border-color: #93c5fd;
  color: #1e40af;
}
.tag-chip.active {
  background: #2563eb;
  border-color: #2563eb;
  color: #fff;
}
.tag-count {
  font-size: 0.85em;
  opacity: 0.75;
  font-weight: 600;
}
.tag-chip.active .tag-count {
  opacity: 0.9;
}

.tag-posts {
  margin-top: 28px;
  padding-top: 16px;
  border-top: 1px solid #e5e7eb;
}
.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.section-header h2 {
  font-size: 16px;
  font-weight: 600;
}
.muted {
  color: #9ca3af;
  font-weight: 400;
  font-size: 14px;
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

.post-list {
  list-style: none;
  padding: 0;
}
.post-list li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 8px;
  border-bottom: 1px solid #f3f4f6;
  cursor: pointer;
}
.post-list li:hover {
  background: #f9fafb;
}
.title {
  flex: 1;
  font-weight: 500;
}
.status {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  background: #e5e7eb;
  text-transform: capitalize;
}
.status.published {
  background: #d1fae5;
  color: #065f46;
}
.status.draft {
  background: #fef3c7;
  color: #92400e;
}

.empty {
  color: #9ca3af;
  text-align: center;
  margin-top: 60px;
  font-size: 14px;
}
</style>
