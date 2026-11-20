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

/**
 * 部署配置（与 Rust deploy() 的 HashMap<String, String> 对应）
 * type 字段决定平台，其余为平台参数：
 * - local:   { type: "local", target }
 * - git:     { type: "git", repo, branch, message }
 * - netlify: { type: "netlify", token, site_id }
 * - vercel:  { type: "vercel", token, project_id, team_id? }
 */
export type DeployConfig = Record<string, string>;

/** 部署目标（持久化在 SiteConfig.deploy_targets 中） */
export interface DeployTarget {
  name: string;
  config: DeployConfig;
}

// ============================================================
// invoke 封装层 — 对 Rust command 做类型安全的包装
// ============================================================

/** 加载所有文章列表（元数据） */
export async function loadPosts(): Promise<PostMeta[]> {
  return invoke<PostMeta[]>("load_posts");
}

/** 按 id 加载完整文章（含正文），编辑器打开已有文章时使用 */
export async function loadPost(id: string): Promise<Post> {
  return invoke<Post>("load_post", { id });
}

/** 删除文章及其资源目录 */
export async function deletePost(id: string): Promise<void> {
  return invoke<void>("delete_post", { id });
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

/** Markdown 批量导入结果（与 Rust ImportSummary 对应） */
export interface ImportSummary {
  /** 成功导入数量 */
  imported: number;
  /** 被跳过的文件路径（非 Markdown 或读取失败） */
  skipped: string[];
  /** 新导入文章元数据 */
  posts: PostMeta[];
}

/**
 * 通过对话框选择多个 Markdown 文件
 * @returns 选中的路径列表；用户取消返回 null
 */
export async function pickMarkdownFiles(): Promise<string[] | null> {
  const selected = await openDialog({
    multiple: true,
    filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
  });
  if (!selected) return null;
  const list = Array.isArray(selected) ? selected : [selected];
  return list as string[];
}

/** 把本地 Markdown 文件批量导入为草稿文章 */
export async function importMarkdown(
  paths: string[],
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_markdown", { paths });
}

/** 生成静态站点到指定目录 */
export async function generateSite(outputDir: string): Promise<string> {
  return invoke<string>("generate_site", { outputDir });
}

/** 读取上次使用的输出目录 */
export async function getOutputDir(): Promise<string> {
  return invoke<string>("get_output_dir");
}

/** 保存输出目录（只回写该字段） */
export async function saveOutputDir(dir: string): Promise<void> {
  return invoke<void>("save_output_dir", { dir });
}

/** 通过系统对话框选择输出目录；用户取消返回 null */
export async function pickOutputDir(): Promise<string | null> {
  const selected = await openDialog({ directory: true, multiple: false });
  return (selected as string) || null;
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

// ============================================================
// 通用主题设置（Logo / 页头 / 页尾 / 广告 / 评论）
// ============================================================

export type CommentSystem =
  | "none"
  | "disqus"
  | "changyan"
  | "livere"
  | "valine"
  | "utterances"
  | "giscus"
  | "custom";

export interface CommentsConfig {
  system: CommentSystem;
  /** 各系统参数键值对 */
  params: Record<string, string>;
}

export interface AdConfig {
  header_html: string;
  sidebar_html: string;
  post_footer_html: string;
  site_footer_html: string;
}

export interface FriendLink {
  name: string;
  url: string;
  description: string;
}

/** 站点导航菜单项（与 Rust MenuItem 字段一致） */
export interface MenuItem {
  label: string;
  url: string;
  /** 内置项（首页/归档/友链），不可修改 URL */
  builtin: boolean;
  /** 是否在导航中显示 */
  enabled: boolean;
}

/** 站点配置（与 Rust SiteConfig 保持字段一致） */
export interface SiteConfig {
  title: string;
  description: string;
  author: string;
  url: string;
  theme: string;
  posts_per_page: number;
  links: FriendLink[];
  /** 站点导航菜单（顺序即展示顺序） */
  menus: MenuItem[];
  // 通用设置
  logo: string;
  header_html: string;
  footer_html: string;
  ads: AdConfig;
  comments: CommentsConfig;
  icp: string;
  /** 静态站点输出目录（预览 / 同步页共享） */
  output_dir: string;
  /** 部署目标列表（多平台部署） */
  deploy_targets: DeployTarget[];
}

/** 获取站点配置 */
export async function getThemeSettings(): Promise<SiteConfig> {
  return invoke<SiteConfig>("get_theme_settings");
}

/** 保存站点配置 */
export async function saveThemeSettings(config: SiteConfig): Promise<void> {
  return invoke<void>("save_theme_settings", { config });
}

/** 保存部署目标列表（只回写 deploy_targets，不影响其他配置） */
export async function saveDeployTargets(
  targets: DeployTarget[],
): Promise<void> {
  return invoke<void>("save_deploy_targets", { targets });
}

/** 获取站点导航菜单 */
export async function getMenus(): Promise<MenuItem[]> {
  return invoke<MenuItem[]>("get_menus");
}

/** 保存站点导航菜单 */
export async function saveMenus(menus: MenuItem[]): Promise<void> {
  return invoke<void>("save_menus", { menus });
}

/** 上传 Logo 图片到 assets 目录，返回相对路径 */
export async function uploadLogo(srcPath: string): Promise<string> {
  return invoke<string>("upload_logo", { srcPath });
}

/** 通过对话框选择并上传 Logo，返回 [相对路径, 原始文件名] 或 null */
export async function pickAndUploadLogo(): Promise<[string, string] | null> {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"] }],
  });
  if (!selected) return null;
  const srcPath = selected as string;
  const filename = srcPath.split(/[\\/]/).pop() ?? "logo";
  const relPath = await uploadLogo(srcPath);
  return [relPath, filename];
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
