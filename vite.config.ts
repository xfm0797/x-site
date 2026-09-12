import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
import viteCompression from "vite-plugin-compression";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [
    vue(),
    // 生产构建时生成 .gz 与 .br 压缩静态资源，供 HTTP 静态部署直接使用
    viteCompression({ algorithm: "gzip", threshold: 1024, deleteOriginFile: false }),
    viteCompression({ algorithm: "brotliCompress", threshold: 1024, deleteOriginFile: false, ext: ".br" }),
  ],

  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  // 生产构建优化
  build: {
    target: "es2021",
    minify: "esbuild", // 比 terser 更快、更小
    chunkSizeWarningLimit: 1024,
    // 移除生产构建中的 console（保留 error/warn 便于排查）
    // 通过 esbuild 选项实现
    esbuild: {
      // drop 仅作用于 esbuild 作为 minifier 时；Vue SFC 由 vue-tsc 编译不受影响
      drop: process.env.NODE_ENV === "production" ? ["console.log", "debugger"] : [],
    },
    rollupOptions: {
      output: {
        // 手动分块：把体积大、更新频率低的依赖拆出去，提高缓存命中率
        // Vite 8 使用 rolldown，manualChunks 必须是函数形式
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("node_modules/vue") || id.includes("node_modules/vue-router") || id.includes("node_modules/pinia")) {
            return "vue";
          }
          if (id.includes("node_modules/@codemirror") || id.includes("node_modules/codemirror")) {
            return "codemirror";
          }
          if (id.includes("node_modules/markdown-it") || id.includes("node_modules/highlight.js")) {
            return "markdown";
          }
          if (id.includes("node_modules/@tauri-apps")) {
            return "tauri";
          }
          return undefined;
        },
      },
    },
  },
}));
