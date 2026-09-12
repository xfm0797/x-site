<script setup lang="ts">
/**
 * Markdown 实时预览组件
 *
 * 使用 markdown-it 渲染正文，highlight.js 为代码块高亮。
 * - 输入防抖 200ms，避免编辑时频繁 re-render
 * - 渲染后的 HTML 通过 v-html 注入；markdown-it 默认不启用 HTML 内联，
 *   但为了图片相对路径可正确显示，外部传入的 cover/src 已是字符串
 * - 代码块在 mounted 后用 highlightElement 手动高亮（性能优于在 mdit 内联）
 */
import { ref, watch, onUpdated, onMounted } from "vue";
import MarkdownIt from "markdown-it";
import hljs from "highlight.js";

const props = defineProps<{
  source: string;
}>();

// 单例 md 实例：开启表格、删除线、任务列表
const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
  breaks: false,
});

// 安全：在新标签页打开外链
const defaultLinkOpen =
  md.renderer.rules.link_open ||
  function (tokens, idx, options, _env, self) {
    return self.renderToken(tokens, idx, options);
  };
md.renderer.rules.link_open = function (tokens, idx, options, env, self) {
  const token = tokens[idx];
  const aIndex = token.attrIndex("target");
  if (aIndex < 0) token.attrPush(["target", "_blank"]);
  else token.attrs![aIndex][1] = "_blank";
  token.attrPush(["rel", "noopener noreferrer"]);
  return defaultLinkOpen(tokens, idx, options, env, self);
};

const rendered = ref("");
let debounceTimer: number | null = null;

function render() {
  rendered.value = md.render(props.source || "");
}

// 防抖渲染
watch(
  () => props.source,
  () => {
    if (debounceTimer !== null) {
      window.clearTimeout(debounceTimer);
    }
    debounceTimer = window.setTimeout(render, 200);
  },
  { immediate: true },
);

// 渲染后给所有 <pre><code> 应用 highlight.js
function highlightAll(container: HTMLElement) {
  const blocks = container.querySelectorAll<HTMLPreElement>(
    "pre code:not([data-hl])",
  );
  blocks.forEach((block) => {
    try {
      hljs.highlightElement(block);
      block.setAttribute("data-hl", "1");
    } catch {
      // ignore unknown language
    }
  });
}

onMounted(() => {
  const el = document.querySelector(".md-preview");
  if (el) highlightAll(el as HTMLElement);
});

onUpdated(() => {
  const el = document.querySelector(".md-preview");
  if (el) highlightAll(el as HTMLElement);
});
</script>

<template>
  <div class="md-preview" v-html="rendered"></div>
</template>

<style scoped>
.md-preview {
  height: 100%;
  width: 100%;
  overflow: auto;
  padding: 16px 24px;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Microsoft YaHei", sans-serif;
  font-size: 15px;
  line-height: 1.75;
  color: #1f2937;
  background: #fff;
}

/* 标题 */
.md-preview :deep(h1) { font-size: 1.8em; margin: 0.6em 0 0.4em; font-weight: 700; }
.md-preview :deep(h2) { font-size: 1.5em; margin: 0.6em 0 0.4em; font-weight: 700; border-bottom: 1px solid #eee; padding-bottom: 0.2em; }
.md-preview :deep(h3) { font-size: 1.25em; margin: 0.6em 0 0.4em; font-weight: 600; }
.md-preview :deep(h4) { font-size: 1.1em; margin: 0.6em 0 0.4em; font-weight: 600; }

/* 段落、列表 */
.md-preview :deep(p) { margin: 0.6em 0; }
.md-preview :deep(ul),
.md-preview :deep(ol) { margin: 0.5em 0; padding-left: 1.8em; }
.md-preview :deep(li) { margin: 0.25em 0; }

/* 链接 */
.md-preview :deep(a) { color: #2563eb; text-decoration: none; }
.md-preview :deep(a:hover) { text-decoration: underline; }

/* 图片 */
.md-preview :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 6px;
  margin: 0.4em 0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}

/* 引用 */
.md-preview :deep(blockquote) {
  margin: 0.6em 0;
  padding: 0.4em 1em;
  border-left: 4px solid #93c5fd;
  background: #eff6ff;
  color: #1e40af;
  border-radius: 0 4px 4px 0;
}
.md-preview :deep(blockquote p) { margin: 0.2em 0; }

/* 行内代码 */
.md-preview :deep(code) {
  font-family: "JetBrains Mono", Consolas, monospace;
  background: #f3f4f6;
  padding: 0.15em 0.4em;
  border-radius: 3px;
  font-size: 0.9em;
  color: #be185d;
}

/* 代码块 */
.md-preview :deep(pre) {
  background: #1f2937;
  color: #f9fafb;
  padding: 14px 16px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 0.8em 0;
}
.md-preview :deep(pre code) {
  background: transparent;
  color: inherit;
  padding: 0;
  font-size: 13px;
  line-height: 1.5;
}

/* 表格 */
.md-preview :deep(table) {
  border-collapse: collapse;
  margin: 0.8em 0;
  width: 100%;
}
.md-preview :deep(th),
.md-preview :deep(td) {
  border: 1px solid #e5e7eb;
  padding: 6px 12px;
  text-align: left;
}
.md-preview :deep(th) { background: #f9fafb; font-weight: 600; }

/* 分隔线 */
.md-preview :deep(hr) {
  border: none;
  border-top: 1px solid #e5e7eb;
  margin: 1.2em 0;
}
</style>
