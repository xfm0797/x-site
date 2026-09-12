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
        }
    }
}
