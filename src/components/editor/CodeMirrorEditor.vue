<script setup lang="ts">
/**
 * CodeMirror 6 的 Vue 3 <script setup> 封装
 *
 * 设计要点：
 * - 单实例：第一次挂载创建 EditorView，之后只通过 dispatch 更新
 * - v-model 单向数据流：内部编辑 → emit 给父级；父级外部修改 → dispatch 同步
 *   避免光标跳动：当新值与当前 doc 相同时跳过 dispatch
 * - 暴露 EditorView 实例，父组件可以调用 view.dispatch / view.scrollPos 等
 * - 暴露 insertText 方法，工具栏按钮调用它插入文本
 */
import {
  watch,
  onMounted,
  onBeforeUnmount,
  shallowRef,
  useTemplateRef,
} from "vue";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { EditorState, Compartment } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { searchKeymap, highlightSelectionMatches } from "@codemirror/search";
import { syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";
import { autocompletion, completionKeymap } from "@codemirror/autocomplete";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    placeholder?: string;
    /** 是否显示行号 */
    lineNumbers?: boolean;
    /** 是否只读 */
    readOnly?: boolean;
  }>(),
  {
    placeholder: "Write your post in Markdown...",
    lineNumbers: true,
    readOnly: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  /** 编辑器准备好后触发，传回 EditorView 实例 */
  ready: [view: EditorView];
}>();

// 用 shallowRef 避免对 EditorView 实例做深层响应式代理（会破坏内部状态）
const viewRef = shallowRef<EditorView | null>(null);
// 宿主 DOM
const hostRef = useTemplateRef<HTMLDivElement>("hostRef");
// 用来标记“这次 dispatch 来自外部，不要再 emit”，防止循环更新
let suppressEmit = false;
// 用 Compartment 让 readOnly 可动态切换
const readOnlyCompartment = new Compartment();

onMounted(() => {
  if (!hostRef.value) return;

  // 更新监听：内部任何事务（含 setValue）触发后，比较 doc 与上次的 modelValue
  const updateListener = EditorView.updateListener.of((vu) => {
    if (vu.docChanged && !suppressEmit) {
      const next = vu.state.doc.toString();
      emit("update:modelValue", next);
    }
  });

  const extensions: any[] = [
    history(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    autocompletion(),
    keymap.of([
      indentWithTab,
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
      ...completionKeymap,
    ]),
    EditorView.lineWrapping,
    EditorView.theme({
      "&": { height: "100%", fontSize: "14px" },
      ".cm-scroller": {
        fontFamily: '"JetBrains Mono", "Cascadia Code", Consolas, monospace',
        lineHeight: "1.6",
      },
      ".cm-content": { padding: "12px 0" },
      ".cm-gutters": { borderRight: "1px solid #e5e7eb" },
    }),
    readOnlyCompartment.of(EditorState.readOnly.of(props.readOnly)),
    EditorView.contentAttributes.of({ "aria-label": "Markdown Editor" }),
    updateListener,
  ];
  if (props.lineNumbers) {
    extensions.splice(1, 0, lineNumbers());
  }

  const state = EditorState.create({
    doc: props.modelValue,
    extensions,
  });

  const view = new EditorView({ state, parent: hostRef.value });
  viewRef.value = view;
  emit("ready", view);
});

onBeforeUnmount(() => {
  viewRef.value?.destroy();
  viewRef.value = null;
});

// 外部 modelValue 变化时同步到编辑器
watch(
  () => props.modelValue,
  (next) => {
    const view = viewRef.value;
    if (!view) return;
    const current = view.state.doc.toString();
    if (next === current) return; // 避免无变化时重置光标
    suppressEmit = true;
    view.dispatch({
      changes: { from: 0, to: current.length, insert: next || "" },
    });
    suppressEmit = false;
  },
);

// readOnly 切换 — 通过 Compartment 重新配置
watch(
  () => props.readOnly,
  (ro) => {
    const view = viewRef.value;
    if (!view) return;
    view.dispatch({
      effects: readOnlyCompartment.reconfigure(EditorState.readOnly.of(ro)),
    });
  },
);

// ============================================================
// 暴露给父组件的方法
// ============================================================
function insertText(text: string, from?: number, to?: number): void {
  const view = viewRef.value;
  if (!view) return;
  const sel = view.state.selection.main;
  view.dispatch({
    changes: { from: from ?? sel.from, to: to ?? sel.to, insert: text },
    selection: { anchor: (from ?? sel.from) + text.length },
    scrollIntoView: true,
    userEvent: "input",
  });
  view.focus();
}

/** 在当前光标位置插入图片 markdown 语法 */
function insertImage(alt: string, url: string): void {
  insertText(`![${alt}](${url})`);
}

/** 在当前光标位置插入链接 */
function insertLink(text: string, url: string): void {
  insertText(`[${text}](${url})`);
}

/** 包裹选中文本（加粗、斜体等），无选中文本时插入占位符并选中 */
function wrapSelection(before: string, after: string = before): void {
  const view = viewRef.value;
  if (!view) return;
  const sel = view.state.selection.main;
  const selected = view.state.sliceDoc(sel.from, sel.to);
  const placeholder = selected || "text";
  view.dispatch({
    changes: { from: sel.from, to: sel.to, insert: before + placeholder + after },
    selection: {
      anchor: sel.from + before.length,
      head: sel.from + before.length + placeholder.length,
    },
    scrollIntoView: true,
    userEvent: "input.wrap",
  });
  view.focus();
}

/** 在行首插入前缀（如 "# " 用于标题，"- " 用于列表） */
function prefixLine(prefix: string): void {
  const view = viewRef.value;
  if (!view) return;
  const sel = view.state.selection.main;
  const doc = view.state.doc.toString();
  // 找到当前行起始
  const lineStart = doc.lastIndexOf("\n", sel.from - 1) + 1;
  view.dispatch({
    changes: { from: lineStart, to: lineStart, insert: prefix },
    selection: { anchor: sel.from + prefix.length },
    scrollIntoView: true,
    userEvent: "input.line",
  });
  view.focus();
}

defineExpose({
  view: viewRef,
  insertText,
  insertImage,
  insertLink,
  wrapSelection,
  prefixLine,
});
</script>

<template>
  <div ref="hostRef" class="cm-host"></div>
</template>

<style scoped>
.cm-host {
  height: 100%;
  width: 100%;
  overflow: hidden;
  text-align: left;
}
.cm-host :deep(.cm-editor) {
  height: 100%;
}
.cm-host :deep(.cm-scroller) {
  overflow: auto;
}
</style>
