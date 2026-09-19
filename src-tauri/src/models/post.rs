use serde::{Deserialize, Serialize};

/// 博客文章完整数据模型
///
/// 字段顺序与前端 `src/api/tauri.ts` 的 `Post` interface 保持一致，
/// 新增字段需通过 `#[serde(default = ...)]` 兼容旧数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub status: String, // "draft" | "published"
    pub tags: Vec<String>,
    pub category: String,
    pub cover: String, // 封面图相对路径，如 "assets/cover-xxxx.png"
    pub created_at: String,
    pub updated_at: String,
}

/// 文章列表元数据（不含正文，减少传输开销）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMeta {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub tags: Vec<String>,
    pub category: String,
    pub cover: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Post {
    /// 生成轻量元数据（用于文章列表）
    pub fn to_meta(&self) -> PostMeta {
        PostMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            slug: self.slug.clone(),
            status: self.status.clone(),
            tags: self.tags.clone(),
            category: self.category.clone(),
            cover: self.cover.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    /// 创建一篇空白草稿（用于编辑器新建文章）
    pub fn new_draft(id: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            title: String::new(),
            slug: String::new(),
            content: String::new(),
            status: "draft".into(),
            tags: Vec::new(),
            category: String::new(),
            cover: String::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// 友情链接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendLink {
    pub name: String,
    pub url: String,
    pub description: String,
}

/// 评论系统类型（支持主流评论服务接入）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentSystem {
    /// 不启用评论
    #[serde(rename = "none")]
    None,
    /// Disqus（短网址）
    Disqus,
    /// 畅言（搜狐）
    Changyan,
    /// LiveRe
    Livere,
    /// Valine（基于 LeanCloud）
    Valine,
    /// Utterances（基于 GitHub Issues）
    Utterances,
    /// Giscus（基于 GitHub Discussions）
    Giscus,
    /// 自定义 HTML 代码
    Custom,
}

impl Default for CommentSystem {
    fn default() -> Self {
        CommentSystem::None
    }
}

/// 评论系统配置（键值对，不同系统所需字段不同）
///
/// 字段约定：
/// - Disqus:        { shortname: "xxx" }
/// - Changyan:      { appid: "xxx", conf: "xxx" }
/// - Livere:        { uid: "xxx", site: "xxx" }
/// - Valine:        { appid: "xxx", appkey: "xxx" }
/// - Utterances:    { repo: "owner/name", label: "blog", theme: "github-light" }
/// - Giscus:        { repo: "owner/name", category: "Announcements", mapping: "pathname" }
/// - Custom:        { html: "<script>...</script>" }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommentsConfig {
    /// 当前选用的评论系统
    #[serde(default)]
    pub system: CommentSystem,
    /// 各系统的参数（键值对，见上方约定）
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

/// 广告位配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdConfig {
    /// 页头广告 HTML（显示在每页顶部 banner 区）
    #[serde(default)]
    pub header_html: String,
    /// 侧边栏广告 HTML
    #[serde(default)]
    pub sidebar_html: String,
    /// 文章底部广告 HTML
    #[serde(default)]
    pub post_footer_html: String,
    /// 全站页脚广告 HTML
    #[serde(default)]
    pub site_footer_html: String,
}

/// 部署目标（多平台部署配置）
///
/// config 键值对约定（type 字段决定平台）：
/// - local:   { type: "local", target: "/path/to/deploy" }
/// - git:     { type: "git", repo: "https://...git", branch: "gh-pages", message: "..." }
/// - netlify: { type: "netlify", token: "xxx", site_id: "xxx" }
/// - vercel:  { type: "vercel", token: "xxx", project_id: "xxx", team_id?: "xxx" }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeployTarget {
    /// 目标名称（如 "GitHub Pages"、"Netlify 主站"）
    #[serde(default)]
    pub name: String,
    /// 部署配置键值对，type 字段选择平台，其余为平台参数
    #[serde(default)]
    pub config: std::collections::HashMap<String, String>,
}

/// 站点配置（保存在 site_config 表的 "main" key 中，以 TOML 序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    pub author: String,
    /// 站点根 URL（用于 RSS / sitemap 的绝对路径），结尾不带斜杠
    pub url: String,
    /// 当前主题名（对应 themes/<theme>/ 目录）
    pub theme: String,
    /// 首页每页文章数
    pub posts_per_page: usize,
    /// 友情链接列表
    pub links: Vec<FriendLink>,
    // ----------------------------------------------------------
    // 通用主题设置项（v0.2 扩展，全部带 default 兼容旧数据）
    // ----------------------------------------------------------
    /// Logo 图片相对路径（如 "assets/logo.png"）。空表示使用纯文字标题
    #[serde(default)]
    pub logo: String,
    /// 自定义页头 HTML（追加在 <body> 顶部 banner 区，用于广告/公告/统计）
    #[serde(default)]
    pub header_html: String,
    /// 自定义页尾 HTML（追加在 </body> 前，用于统计/备案/广告）
    #[serde(default)]
    pub footer_html: String,
    /// 广告位配置
    #[serde(default)]
    pub ads: AdConfig,
    /// 评论系统配置
    #[serde(default)]
    pub comments: CommentsConfig,
    /// ICP 备案号（中国大陆站点底部显示）
    #[serde(default)]
    pub icp: String,
    /// 部署目标列表（多平台部署，见 DeployTarget 约定）
    #[serde(default)]
    pub deploy_targets: Vec<DeployTarget>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "My Blog".into(),
            description: "A static blog powered by x-site".into(),
            author: "XFM".into(),
            url: String::new(),
            theme: "default".into(),
            posts_per_page: 10,
            links: Vec::new(),
            logo: String::new(),
            header_html: String::new(),
            footer_html: String::new(),
            ads: AdConfig::default(),
            comments: CommentsConfig::default(),
            icp: String::new(),
            deploy_targets: Vec::new(),
        }
    }
}
