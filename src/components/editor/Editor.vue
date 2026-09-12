<script setup lang="ts">
import { ref, watch } from "vue";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const text = ref(props.modelValue);

watch(text, (val) => emit("update:modelValue", val));
watch(() => props.modelValue, (val) => (text.value = val));
</script>

<template>
  <textarea
    v-model="text"
    class="editor"
    placeholder="Write your post in Markdown..."
  ></textarea>
</template>

<style scoped>
.editor {
  width: 100%;
  min-height: 420px;
  padding: 12px;
  font-family: "JetBrains Mono", "Cascadia Code", Consolas, monospace;
  font-size: 14px;
  line-height: 1.6;
  border: 1px solid #ddd;
  border-radius: 6px;
  resize: vertical;
  outline: none;
}
.editor:focus {
  border-color: #4a9eff;
}
</style>
