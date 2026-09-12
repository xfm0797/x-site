// 静态站点生成器主模块
//
// 模块组织：
//   mod.rs       — 主入口 generate()、协调各子模块、并发渲染、进度推送
//   context.rs   — SiteContext / PostContext 等模板上下文结构
//   render.rs    — pulldown-cmark Markdown → HTML、Tera 模板渲染
//   assets.rs    — 主题资源复制（css/js/img）
//   feed.rs      — sitemap.xml / RSS feed.xml / robots.txt 生成

pub mod assets;
pub mod context;
pub mod feed;
pub mod render;

use crate::db::DbState;
use crate::models::post::{Post, SiteConfig};
use context::{SiteContext, build_post_context};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

// ============================================================
// 主入口
// ============================================================

/// 生成静态站点到 output_dir
///
/// 步骤：
/// 1. 加载已发布文章 + 站点配置
/// 2. 加载主题模板（Tera 实例）
/// 3. 复制主题 assets 到 output/assets
/// 4. 并发渲染文章页（rayon）
/// 5. 渲染首页（分页）、归档页、标签页、分类页、友情链接页
/// 6. 生成 sitemap.xml、feed.xml、robots.txt
///
/// 进度通过 Tauri 事件 `generate://progress` 推送，payload 形如：
///   { "current": 3, "total": 10, "percent": 30, "file": "posts/hello.html" }
pub fn generate(
    app: &AppHandle,
    db: &DbState,
    output_dir: &str,
) -> Result<(), String> {
    // ----------------------------------------------------------
    // 1. 加载数据
    // ----------------------------------------------------------
    emit_progress(app, 0, 1, "Loading posts...");
    let posts = load_published_posts(db)?;
    let site_config = load_site_config(db)?;
    let total = posts.len() + 6; // 文章 + 首页/归档/标签/分类/links/feed
    let mut current = 0usize;

    if posts.is_empty() {
        return Err("No published posts found".into());
    }

    // ----------------------------------------------------------
    // 2. 准备 Tera 模板
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Loading theme templates...");
    let theme_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("themes")
        .join(&site_config.theme);
    let tera = render::load_theme(&theme_dir)?;

    // 输出目录结构
    let output_path = Path::new(output_dir);
    let posts_dir = output_path.join("posts");
    let tags_dir = output_path.join("tags");
    let cats_dir = output_path.join("categories");
    let page_dir = output_path.join("page");
    for d in [&posts_dir, &tags_dir, &cats_dir, &page_dir] {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }

    // ----------------------------------------------------------
    // 3. 复制主题 assets
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Copying theme assets...");
    let theme_assets_dir = theme_dir.join("assets");
    let out_assets_dir = output_path.join("assets");
    if theme_assets_dir.exists() {
        assets::copy_dir(&theme_assets_dir, &out_assets_dir)
            .map_err(|e| format!("copy assets: {e}"))?;
    }

    // ----------------------------------------------------------
    // 4. 并发渲染文章页
    // ----------------------------------------------------------
    // 把 SiteContext 提前构造好（Arc 共享），每个文章只需追加自己的字段
    let site_ctx = Arc::new(SiteContext::new(&site_config));
    // 共享 Tera 实例（Tera 内部用 RwLock，可并发只读渲染）
    let tera = Arc::new(tera);

    // 收集结果到 Vec（按原顺序保持稳定）
    let results: Vec<Result<(String, String), String>> = posts
        .par_iter()
        .map(|post| {
            let html = render::render_markdown(&post.content);
            let ctx = build_post_context(&site_ctx, post, &html);
            let rendered = render::render_template(&tera, "post.tera", &ctx)
                .map_err(|e| format!("post {}: {}", post.id, e))?;
            let filename = format!("{}.html", post_slug_or_id(post));
            Ok((filename, rendered))
        })
        .collect();

    // 写入文件（顺序写，避免 rayon 内部 IO 交错）
    for res in results {
        let (filename, html) = res?;
        let file_path = posts_dir.join(&filename);
        fs::write(&file_path, html).map_err(|e| e.to_string())?;
        current += 1;
        emit_progress(app, current, total, &format!("posts/{}", filename));
    }

    // ----------------------------------------------------------
    // 5. 首页（分页）
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Rendering index pages...");
    let per_page = site_config.posts_per_page.max(1);
    let total_pages = (posts.len() + per_page - 1) / per_page;
    for page_num in 1..=total_pages {
        let start = (page_num - 1) * per_page;
        let end = (start + per_page).min(posts.len());
        let page_posts = &posts[start..end];
        let html = render::render_index_page(
            &tera,
            &site_ctx,
            page_posts,
            page_num,
            total_pages,
        )?;
        if page_num == 1 {
            fs::write(output_path.join("index.html"), html)
                .map_err(|e| e.to_string())?;
        } else {
            fs::write(page_dir.join(format!("{}.html", page_num)), html)
                .map_err(|e| e.to_string())?;
        }
    }
    current += 1;

    // ----------------------------------------------------------
    // 6. 归档页
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Rendering archive...");
    let archive_html = render::render_archive(&tera, &site_ctx, &posts)?;
    fs::write(output_path.join("archive.html"), archive_html)
        .map_err(|e| e.to_string())?;
    current += 1;

    // ----------------------------------------------------------
    // 7. 标签页（按 tag 分组）
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Rendering tag pages...");
    let mut tags_map: BTreeMap<String, Vec<&Post>> = BTreeMap::new();
    for p in &posts {
        for t in &p.tags {
            tags_map.entry(t.clone()).or_default().push(p);
        }
    }
    for (tag, tag_posts) in &tags_map {
        let html = render::render_tag_page(
            &tera,
            &site_ctx,
            tag,
            tag_posts,
        )?;
        let filename = format!("{}.html", slugify::slugify(tag));
        fs::write(tags_dir.join(&filename), html).map_err(|e| e.to_string())?;
    }
    current += 1;

    // ----------------------------------------------------------
    // 8. 分类页
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Rendering category pages...");
    let mut cats_map: BTreeMap<String, Vec<&Post>> = BTreeMap::new();
    for p in &posts {
        if !p.category.is_empty() {
            cats_map.entry(p.category.clone()).or_default().push(p);
        }
    }
    for (cat, cat_posts) in &cats_map {
        let html = render::render_category_page(
            &tera,
            &site_ctx,
            cat,
            cat_posts,
        )?;
        let filename = format!("{}.html", slugify::slugify(cat));
        fs::write(cats_dir.join(&filename), html).map_err(|e| e.to_string())?;
    }
    current += 1;

    // ----------------------------------------------------------
    // 9. 友情链接页
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Rendering links page...");
    let links_html = render::render_links_page(&tera, &site_ctx)?;
    fs::write(output_path.join("links.html"), links_html)
        .map_err(|e| e.to_string())?;
    current += 1;

    // ----------------------------------------------------------
    // 10. sitemap / RSS / robots
    // ----------------------------------------------------------
    emit_progress(app, current, total, "Generating sitemap & feed...");
    feed::write_sitemap(output_path, &site_config, &posts)
        .map_err(|e| e.to_string())?;
    feed::write_rss(output_path, &site_config, &posts)
        .map_err(|e| e.to_string())?;
    feed::write_robots(output_path, &site_config).map_err(|e| e.to_string())?;
    current += 1;

    emit_progress(app, total, total, "Done");
    Ok(())
}

// ============================================================
// 数据加载
// ============================================================

fn load_published_posts(db: &DbState) -> Result<Vec<Post>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, slug, content, status, tags,
                   category, cover, created_at, updated_at
             FROM posts WHERE status = 'published'
             ORDER BY datetime(created_at) DESC",
        )
        .map_err(|e| e.to_string())?;

    let posts = stmt
        .query_map([], |row| {
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(Post {
                id: row.get(0)?,
                title: row.get(1)?,
                slug: row.get(2)?,
                content: row.get(3)?,
                status: row.get(4)?,
                tags,
                category: row.get(6)?,
                cover: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(posts)
}

/// 从 site_config 表读取主配置（key="main"）
/// 缺失时返回默认值
fn load_site_config(db: &DbState) -> Result<SiteConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM site_config WHERE key = ?1")
        .map_err(|e| e.to_string())?;
    let row: Option<String> = stmt
        .query_row([&"main"], |r| r.get(0))
        .ok();
    match row {
        Some(toml_str) => {
            toml::from_str::<SiteConfig>(&toml_str)
                .map_err(|e| format!("parse site_config: {e}"))
        }
        None => Ok(SiteConfig::default()),
    }
}

// ============================================================
// 工具
// ============================================================

/// 文章 URL slug：优先使用 slug，否则用 id
fn post_slug_or_id(post: &Post) -> &str {
    if post.slug.is_empty() {
        &post.id
    } else {
        &post.slug
    }
}

/// 推送进度事件
fn emit_progress(app: &AppHandle, current: usize, total: usize, file: &str) {
    let percent = if total == 0 {
        100
    } else {
        ((current as f64 / total as f64) * 100.0) as u32
    };
    let _ = app.emit(
        "generate://progress",
        serde_json::json!({
            "current": current,
            "total": total,
            "percent": percent,
            "file": file,
        }),
    );
}

// 临时内联 slugify（避免引入额外 crate）
mod slugify {
    pub fn slugify(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else if c.is_whitespace() || c == '-' {
                    '-'
                } else {
                    '\0'
                }
            })
            .filter(|c| *c != '\0')
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }
}
