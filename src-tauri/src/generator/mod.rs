// 静态站点生成器主模块
//
// 模块组织：
//   mod.rs       — 主入口 generate()、协调各子模块、并发渲染、进度推送
//   context.rs   — SiteContext / PostContext 等模板上下文结构
//   render.rs    — pulldown-cmark Markdown → HTML、Tera 模板渲染
//   assets.rs    — 主题资源复制（css/js/img）
//   feed.rs      — sitemap.xml / RSS feed.xml / robots.txt 生成
//
// generator 模块**不依赖 tauri crate**：路径 / 进度上报通过 `GeneratorPaths` 和
// `ProgressSink` trait 注入，这样可以脱离框架直接写 `cargo test`。
// Tauri 命令层（commands/generate.rs）负责把 AppHandle 适配为上述接口。

pub mod assets;
pub mod context;
pub mod feed;
pub mod render;

use crate::db::{self, DbState};
use crate::error::{AppError, AppResult};
use crate::models::post::Post;
use context::{SiteContext, build_post_context};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ============================================================
// 依赖注入接口（解耦 generator 与 Tauri 框架）
// ============================================================

/// 生成器所需的文件系统路径，由上层（Tauri command）从 AppHandle 中组装
pub struct GeneratorPaths {
    /// 主题目录根：`<app_data_dir>/themes/`
    pub themes_dir: PathBuf,
    /// 用户资源目录（文章插图、Logo 等）：`<app_data_dir>/assets/`
    pub user_assets_dir: PathBuf,
}

/// 进度上报抽象：Tauri 命令层用它推送事件，测试里可以用空实现或收集到 Vec
pub trait ProgressSink: Send + Sync {
    fn emit(&self, current: usize, total: usize, file: &str);
}

/// 空 sink（用于测试或生成过程中不关心进度）
#[allow(dead_code)]
pub struct NullProgress;
impl ProgressSink for NullProgress {
    fn emit(&self, _current: usize, _total: usize, _file: &str) {}
}

// ============================================================
// 主入口
// ============================================================

/// 清理生成器管理的产物目录与文件（输出目录中的其他文件保持不变）
fn clean_output(output_path: &Path) {
    const MANAGED_DIRS: &[&str] = &["posts", "tags", "categories", "page", "assets"];
    const MANAGED_FILES: &[&str] = &[
        "index.html",
        "archive.html",
        "links.html",
        "sitemap.xml",
        "feed.xml",
        "robots.txt",
    ];
    for d in MANAGED_DIRS {
        let p = output_path.join(d);
        if p.exists() {
            let _ = fs::remove_dir_all(&p);
        }
    }
    for f in MANAGED_FILES {
        let p = output_path.join(f);
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
    }
}

/// 生成静态站点到 output_dir
///
/// 步骤：
/// 1. 加载已发布文章 + 站点配置（含 slug 冲突检测）
/// 2. 加载主题模板（Tera 实例），清理旧产物
/// 3. 复制主题 assets 与用户资源（文章插图、Logo）到 output/assets
/// 4. 并发渲染文章页（rayon）
/// 5. 渲染首页（分页）、归档页、标签页、分类页、友情链接页
/// 6. 生成 sitemap.xml、feed.xml、robots.txt
pub fn generate(
    paths: &GeneratorPaths,
    progress: &dyn ProgressSink,
    db: &DbState,
    output_dir: &str,
) -> AppResult<()> {
    // ----------------------------------------------------------
    // 1. 加载数据
    // ----------------------------------------------------------
    progress.emit(0, 1, "Loading posts...");
    let posts = db::posts::load_published(db)?;
    let site_config = db::config::load(db)?;
    let total = posts.len() + 6; // 文章 + 首页/归档/标签/分类/links/feed
    let mut current = 0usize;

    if posts.is_empty() {
        return Err(AppError::Validation("No published posts found".into()));
    }

    // 同 slug 的文章会互相覆盖 HTML，生成前直接报错并指出冲突
    let mut seen_slugs = std::collections::HashSet::new();
    for post in &posts {
        let slug = post_slug_or_id(post);
        if !seen_slugs.insert(slug) {
            return Err(AppError::Validation(format!(
                "Duplicate post slug '{}': please change the slug of one of the posts before generating",
                slug
            )));
        }
    }

    // ----------------------------------------------------------
    // 2. 准备 Tera 模板
    // ----------------------------------------------------------
    progress.emit(current, total, "Loading theme templates...");
    let theme_dir = paths.themes_dir.join(&site_config.theme);
    let tera = render::load_theme(&theme_dir)?;

    // 输出目录结构（先清理生成器管理的旧产物，避免删文/改 slug 后残留幽灵页面）
    let output_path = Path::new(output_dir);
    clean_output(output_path);
    let posts_dir = output_path.join("posts");
    let tags_dir = output_path.join("tags");
    let cats_dir = output_path.join("categories");
    let page_dir = output_path.join("page");
    for d in [&posts_dir, &tags_dir, &cats_dir, &page_dir] {
        fs::create_dir_all(d)?;
    }

    // ----------------------------------------------------------
    // 3. 复制主题 assets
    // ----------------------------------------------------------
    progress.emit(current, total, "Copying theme assets...");
    let theme_assets_dir = theme_dir.join("assets");
    let out_assets_dir = output_path.join("assets");
    if theme_assets_dir.exists() {
        assets::copy_dir(&theme_assets_dir, &out_assets_dir)
            .map_err(AppError::Io)?;
    }
    // 文章插图、Logo 等用户资源（app_data/assets/）一并复制，修复生成站图片 404
    if paths.user_assets_dir.exists() {
        assets::copy_dir(&paths.user_assets_dir, &out_assets_dir)
            .map_err(AppError::Io)?;
    }

    // ----------------------------------------------------------
    // 4. 并发渲染文章页
    // ----------------------------------------------------------
    let site_ctx = Arc::new(SiteContext::new(&site_config));
    let tera = Arc::new(tera);

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

    for res in results {
        let (filename, html) = res.map_err(AppError::Template)?;
        let file_path = posts_dir.join(&filename);
        fs::write(&file_path, html)?;
        current += 1;
        progress.emit(current, total, &format!("posts/{}", filename));
    }

    // ----------------------------------------------------------
    // 5. 首页（分页）
    // ----------------------------------------------------------
    progress.emit(current, total, "Rendering index pages...");
    let per_page = site_config.posts_per_page.max(1);
    let total_pages = posts.len().div_ceil(per_page);
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
            fs::write(output_path.join("index.html"), html)?;
        } else {
            fs::write(page_dir.join(format!("{}.html", page_num)), html)?;
        }
    }
    current += 1;

    // ----------------------------------------------------------
    // 6. 归档页
    // ----------------------------------------------------------
    progress.emit(current, total, "Rendering archive...");
    let archive_html = render::render_archive(&tera, &site_ctx, &posts)?;
    fs::write(output_path.join("archive.html"), archive_html)?;
    current += 1;

    // ----------------------------------------------------------
    // 7. 标签页（按 tag 分组）
    // ----------------------------------------------------------
    progress.emit(current, total, "Rendering tag pages...");
    let mut tags_map: BTreeMap<String, Vec<&Post>> = BTreeMap::new();
    for p in &posts {
        for t in &p.tags {
            tags_map.entry(t.clone()).or_default().push(p);
        }
    }
    for (tag, tag_posts) in &tags_map {
        let html =
            render::render_tag_page(&tera, &site_ctx, tag, tag_posts)?;
        let filename = format!("{}.html", slugify(tag));
        fs::write(tags_dir.join(&filename), html)?;
    }
    current += 1;

    // ----------------------------------------------------------
    // 8. 分类页
    // ----------------------------------------------------------
    progress.emit(current, total, "Rendering category pages...");
    let mut cats_map: BTreeMap<String, Vec<&Post>> = BTreeMap::new();
    for p in &posts {
        if !p.category.is_empty() {
            cats_map.entry(p.category.clone()).or_default().push(p);
        }
    }
    for (cat, cat_posts) in &cats_map {
        let html =
            render::render_category_page(&tera, &site_ctx, cat, cat_posts)?;
        let filename = format!("{}.html", slugify(cat));
        fs::write(cats_dir.join(&filename), html)?;
    }
    current += 1;

    // ----------------------------------------------------------
    // 9. 友情链接页
    // ----------------------------------------------------------
    progress.emit(current, total, "Rendering links page...");
    let links_html = render::render_links_page(&tera, &site_ctx)?;
    fs::write(output_path.join("links.html"), links_html)?;
    current += 1;

    // ----------------------------------------------------------
    // 10. sitemap / RSS / robots
    // ----------------------------------------------------------
    progress.emit(current, total, "Generating sitemap & feed...");
    feed::write_sitemap(output_path, &site_config, &posts)?;
    feed::write_rss(output_path, &site_config, &posts)?;
    feed::write_robots(output_path, &site_config)?;
    // current 在 sitemap/RSS/robots 生成时无独立进度点，最终 emit(total, total, ...) 已兜底

    progress.emit(total, total, "Done");
    Ok(())
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

/// 临时内联 slugify（避免引入额外 crate）
fn slugify(text: &str) -> String {
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
