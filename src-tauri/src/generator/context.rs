//! 模板上下文结构
//!
//! 这些结构会被 serde 序列化后传给 Tera，在模板中通过
//! `{{ site.title }}` / `{{ post.title }}` / `{{ posts }}` 等方式访问。

use crate::models::post::{FriendLink, Post, SiteConfig};
use serde::Serialize;

/// 站点级上下文（每次渲染都注入）
///
/// 在 Tera 模板里访问：`{{ site.title }}`、`{{ site.author }}`、`{{ site.links }}`
#[derive(Debug, Clone, Serialize)]
pub struct SiteContext {
    pub title: String,
    pub description: String,
    pub author: String,
    pub url: String,
    pub theme: String,
    pub posts_per_page: usize,
    pub links: Vec<FriendLink>,
    /// 当前年份（页脚用）
    pub year: i32,
}

impl SiteContext {
    pub fn new(cfg: &SiteConfig) -> Self {
        Self {
            title: cfg.title.clone(),
            description: cfg.description.clone(),
            author: cfg.author.clone(),
            url: cfg.url.clone(),
            theme: cfg.theme.clone(),
            posts_per_page: cfg.posts_per_page,
            links: cfg.links.clone(),
            year: chrono::Utc::now().year(),
        }
    }
}

/// 文章页上下文
#[derive(Debug, Serialize)]
pub struct PostContext<'a> {
    pub site: &'a SiteContext,
    pub post: PostData<'a>,
}

/// 单篇文章数据（独立结构避免 Post 暴露内部字段）
#[derive(Debug, Serialize)]
pub struct PostData<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub slug: &'a str,
    pub tags: &'a [String],
    pub category: &'a str,
    pub cover: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    /// 已渲染好的 HTML（pulldown-cmark + syntect 输出）
    pub content: String,
    /// 文章绝对 URL（如 "/posts/hello.html"）
    pub url: String,
}

pub fn build_post_context<'a>(
    site: &'a SiteContext,
    post: &'a Post,
    rendered_html: &str,
) -> PostContext<'a> {
    let slug = if post.slug.is_empty() {
        &post.id
    } else {
        &post.slug
    };
    let url = format!("/posts/{}.html", slug);
    PostContext {
        site,
        post: PostData {
            id: &post.id,
            title: &post.title,
            slug,
            tags: &post.tags,
            category: &post.category,
            cover: &post.cover,
            created_at: &post.created_at,
            updated_at: &post.updated_at,
            content: rendered_html.to_string(),
            url,
        },
    }
}

/// 列表项（首页/归档/标签/分类用）
#[derive(Debug, Serialize)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub category: String,
    pub tags: Vec<String>,
    pub cover: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    /// 纯文本摘要（前 200 字符）
    pub excerpt: String,
}

impl ListItem {
    pub fn from_post(post: &Post) -> Self {
        let slug = if post.slug.is_empty() {
            post.id.clone()
        } else {
            post.slug.clone()
        };
        let url = format!("/posts/{}.html", slug);
        let excerpt = plain_text_excerpt(&post.content, 200);
        Self {
            id: post.id.clone(),
            title: post.title.clone(),
            slug,
            category: post.category.clone(),
            tags: post.tags.clone(),
            cover: post.cover.clone(),
            created_at: post.created_at.clone(),
            updated_at: post.updated_at.clone(),
            url,
            excerpt,
        }
    }
}

/// 首页 / 分页上下文
#[derive(Debug, Serialize)]
pub struct IndexContext<'a> {
    pub site: &'a SiteContext,
    pub posts: Vec<ListItem>,
    pub page: usize,
    pub total_pages: usize,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
}

/// 归档页上下文
#[derive(Debug, Serialize)]
pub struct ArchiveContext<'a> {
    pub site: &'a SiteContext,
    /// 按年份分组：[(year, [list_item...])]
    pub groups: Vec<(String, Vec<ListItem>)>,
}

/// 标签页上下文
#[derive(Debug, Serialize)]
pub struct TagContext<'a> {
    pub site: &'a SiteContext,
    pub tag: String,
    pub posts: Vec<ListItem>,
}

/// 分类页上下文
#[derive(Debug, Serialize)]
pub struct CategoryContext<'a> {
    pub site: &'a SiteContext,
    pub category: String,
    pub posts: Vec<ListItem>,
}

/// 友情链接页上下文
#[derive(Debug, Serialize)]
pub struct LinksContext<'a> {
    pub site: &'a SiteContext,
}

// ============================================================
// 工具
// ============================================================

/// 把 Markdown 转为纯文本摘要（去除标记）
fn plain_text_excerpt(markdown: &str, max_chars: usize) -> String {
    let mut text = String::new();
    for line in markdown.lines() {
        // 跳过标题、分隔线、代码块
        if line.starts_with('#') || line.starts_with("---") || line.starts_with("```") {
            continue;
        }
        let stripped = line
            .trim_start_matches('-')
            .trim_start_matches('>')
            .trim();
        text.push_str(stripped);
        text.push(' ');
        if text.chars().count() >= max_chars {
            break;
        }
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > max_chars {
        let s: String = chars.iter().take(max_chars).collect();
        format!("{}…", s)
    } else {
        text.trim().to_string()
    }
}

// 引入 chrono 的 Datelike（用于 year()）
use chrono::Datelike;
