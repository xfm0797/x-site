import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // store 里用 window.setTimeout 做防抖，需要 DOM 环境
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
