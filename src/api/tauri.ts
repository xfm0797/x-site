import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

// ============================================================
// 类型定义 — 与 Rust models/post.rs 保持一致
// ============================================================

export interface Post {
  id: string;
  title: string;
  slug: string;
  content: string;
  status: "draft" | "published";
  tags: string[];
  category: string;
  /** 封面图相对路径，如 "assets/<post_id>/cover-xxxx.png" */
  cover: string;
  created_at: string;
  updated_at: string;
}

export interface PostMeta {
  id: string;
  title: string;
  slug: string;
  status: string;
  tags: string[];
  category: string;
  cover: string;
  created_at: string;
  updated_at: string;
}

export interface DeployConfig {
  type: "local" | "git";
  target?: string;
  repo?: string;
  branch?: string;
  message?: string;
}

// ============================================================
// invoke 封装层 — 对 Rust command 做类型安全的包装
// ============================================================

/** 加载所有文章列表（元数据） */
export async function loadPosts(): Promise<PostMeta[]> {
  return invoke<PostMeta[]>("load_posts");
}

/** 保存文章（新增或更新） */
export async function savePost(post: Post): Promise<Post> {
  return invoke<Post>("save_post", { post });
}

/**
 * 导入本地图片到文章资源目录
 * @returns 可在 Markdown 中引用的相对路径，如 "assets/<post_id>/img-xxx.png"
 */
export async function importImage(
  postId: string,
  srcPath: string,
): Promise<string> {
  return invoke<string>("import_image", { postId, srcPath });
}

/**
 * 通过 Tauri dialog 打开图片选择器，并将图片导入资源目录
 * @returns 成功时返回 [相对路径, 原始文件名]；用户取消则返回 null
 */
export async function pickAndImportImage(
  postId: string,
): Promise<[string, string] | null> {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg"] }],
  });
  if (!selected) return null;
  const srcPath = selected as string;
  const filename = srcPath.split(/[\\/]/).pop() ?? "image";
  const relPath = await importImage(postId, srcPath);
  return [relPath, filename];
}

/** 生成静态站点到指定目录 */
export async function generateSite(outputDir: string): Promise<string> {
  return invoke<string>("generate_site", { outputDir });
}

/** 部署站点 */
export async function deploy(
  siteDir: string,
  config: DeployConfig,
): Promise<string> {
  return invoke<string>("deploy", { siteDir, config });
}

// ============================================================
// 主题管理
// ============================================================

export interface Theme {
  name: string;
  display_name: string;
  author: string;
  description: string;
  version: string;
  screenshot: string;
  homepage?: string;
  license?: string;
  /** 是否当前激活 */
  active: boolean;
}

/** 列出所有已安装主题 */
export async function listThemes(): Promise<Theme[]> {
  return invoke<Theme[]>("list_themes");
}

/** 切换当前主题 */
export async function applyTheme(name: string): Promise<void> {
  return invoke<void>("apply_theme", { name });
}

/** 从 zip 包导入主题，返回主题名 */
export async function importTheme(zipPath: string): Promise<string> {
  return invoke<string>("import_theme", { zipPath });
}

// ============================================================
// 本地预览服务器
// ============================================================

/** 启动预览服务，返回端口 */
export async function startPreview(outputDir: string): Promise<number> {
  return invoke<number>("start_preview", { outputDir });
}

/** 停止预览服务 */
export async function stopPreview(): Promise<void> {
  return invoke<void>("stop_preview");
}

/** 获取当前预览端口（0 表示未启动） */
export async function getPreviewPort(): Promise<number> {
  return invoke<number>("get_preview_port");
}

/** 生成 slug：中文转拼音首字母已超出范围，这里用 ASCII 化 + 连字符 */
export function slugify(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, "") // 移除标点
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 60);
}

/** 创建一篇空白草稿（用于编辑器新建文章） */
export function newDraft(id: string): Post {
  const now = new Date().toISOString();
  return {
    id,
    title: "",
    slug: "",
    content: "",
    status: "draft",
    tags: [],
    category: "",
    cover: "",
    created_at: now,
    updated_at: now,
  };
}
