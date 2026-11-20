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

/// 清理生成器管理的产物
/// - posts/tags/categories/page 目录下**未在 keep_slugs 中的 HTML** 会被删除
/// - 顶层 managed files 每次都删（索引/归档/feed 等依赖文章列表，变化频繁）
/// - assets 目录只在主题变更时清理（外部资源目录跳过，用户可能有自定义文件）
fn clean_output(output_path: &Path, keep_slugs: &std::collections::HashSet<String>, theme_changed: bool) {
    let posts_dir = output_path.join("posts");
    if posts_dir.exists() {
        // 遍历 posts/*.html，删不在 keep_slugs 里的
        if let Ok(dir) = fs::read_dir(&posts_dir) {
            for entry in dir.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".html") {
                    let slug = name.trim_end_matches(".html");
                    if !keep_slugs.contains(slug) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    // tags/categories/page 目录整体删（数量少，重建成本低）
    for d in ["tags", "categories", "page"] {
        let p = output_path.join(d);
        if p.exists() {
            let _ = fs::remove_dir_all(&p);
        }
    }

    // assets 主题变了才清（保留用户可能放进去的自定义文件）
    if theme_changed {
        let assets = output_path.join("assets");
        if assets.exists() {
            let _ = fs::remove_dir_all(&assets);
        }
    }

    const MANAGED_FILES: &[&str] = &[
        "index.html",
        "archive.html",
        "links.html",
        "sitemap.xml",
        "feed.xml",
        "robots.txt",
    ];
    for f in MANAGED_FILES {
        let p = output_path.join(f);
        if p.exists() {
            let _ = fs::remove_file(&p);
        }
    }
}

/// 增量生成状态（保存在 output/.gen_state.json）
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct GenState {
    /// 上次生成时的主题名——不同主题必须全量重建
    theme: String,
    /// 每篇文章的 { slug, content_hash }，用于跳过未变文章的渲染
    #[serde(default)]
    posts: std::collections::BTreeMap<String, PostHash>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PostHash {
    slug: String,
    #[serde(rename = "h")]
    h: String,
}

fn content_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())[..16].to_string() // 前 16 hex 字符足够
}

const STATE_FILE: &str = ".gen_state.json";

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
        if !seen_slugs.insert(slug.to_string()) {
            return Err(AppError::Validation(format!(
                "Duplicate post slug '{}': please change the slug of one of the posts before generating",
                slug
            )));
        }
    }

    // ----------------------------------------------------------
    // 2.5. 加载增量状态（output/.gen_state.json）
    // ----------------------------------------------------------
    let output_path = Path::new(output_dir);
    let state_path = output_path.join(STATE_FILE);
    let prev_state: GenState = state_path
        .exists()
        .then(|| fs::read_to_string(&state_path))
        .transpose()
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let theme_changed = prev_state.theme.is_empty() || prev_state.theme != site_config.theme;

    // ----------------------------------------------------------
    // 3. 输出目录准备
    // ----------------------------------------------------------
    progress.emit(current, total, "Preparing output directory...");
    fs::create_dir_all(output_path)?;

    // 智能清理：删除消失的 slug HTML、tags/cats/page、顶层 managed files
    clean_output(output_path, &seen_slugs, theme_changed);

    let posts_dir = output_path.join("posts");
    let tags_dir = output_path.join("tags");
    let cats_dir = output_path.join("categories");
    let page_dir = output_path.join("page");
    for d in [&posts_dir, &tags_dir, &cats_dir, &page_dir] {
        fs::create_dir_all(d)?;
    }

    // ----------------------------------------------------------
    // 4. 准备 Tera 模板 + 复制 assets（主题变更时 assets 目录已被清理）
    // ----------------------------------------------------------
    progress.emit(current, total, "Loading theme templates...");
    let theme_dir = paths.themes_dir.join(&site_config.theme);
    let tera = render::load_theme(&theme_dir)?;

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
    // 5. 并发渲染文章页（增量模式）
    // ----------------------------------------------------------
    let site_ctx = Arc::new(SiteContext::new(&site_config));
    let tera = Arc::new(tera);
    let mut next_state = GenState {
        theme: site_config.theme.clone(),
        posts: BTreeMap::new(),
    };

    // 对每篇文章算新 hash；与 prev_state 同 slug 的 hash 相同且文件存在 → 跳过
    // 四个字段：(slug, html_opt, post_id, hash)
    #[allow(clippy::type_complexity)]
    let results: Vec<AppResult<(String, Option<String>, String, String)>> = posts
        .par_iter()
        .map(|post| {
            let slug = post_slug_or_id(post);
            let slug_owned = slug.to_string();
            let hash = content_hash(&post.content);

            // 跳过条件：主题未变 + 同 slug 的 hash 相同 + 旧 HTML 文件存在
            let should_skip = !theme_changed
                && prev_state
                    .posts
                    .get(&post.id)
                    .map(|ph| ph.slug == slug && ph.h == hash)
                    .unwrap_or(false)
                && posts_dir.join(format!("{}.html", slug)).exists();

            if should_skip {
                // 跳过渲染（Markdown + Tera），HTML 文件保持不变
                return Ok((slug_owned, None, post.id.clone(), hash));
            }

            let html = render::render_markdown(&post.content);
            let ctx = build_post_context(&site_ctx, post, &html);
            let rendered = render::render_template(&tera, "post.tera", &ctx)
                .map_err(|e| AppError::Template(format!("post {}: {}", post.id, e)))?;
            Ok((slug_owned, Some(rendered), post.id.clone(), hash))
        })
        .collect();

    for res in results {
        let (slug, html_opt, post_id, hash) = res?;
        // 在外部闭包外安全地更新 next_state（rayon 的 par_iter 闭包不能做 FnMut）
        next_state
            .posts
            .insert(post_id, PostHash { slug: slug.clone(), h: hash });
        let file_path = posts_dir.join(format!("{}.html", slug));
        if let Some(html) = html_opt {
            fs::write(&file_path, html)?;
        }
        current += 1;
        progress.emit(current, total, &format!("posts/{}", slug));
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

    // ----------------------------------------------------------
    // 11. 写回增量状态（下一次生成时用于跳过未变文章）
    // ----------------------------------------------------------
    // 即使渲染有跳过，BTreeMap 顺序也是稳定的，JSON 可 diff
    let state_json = serde_json::to_string_pretty(&next_state)
        .map_err(|e| AppError::Db(format!("gen_state: {e}")))?;
    fs::write(&state_path, state_json).map_err(AppError::Io)?;

    progress.emit(total, total, "Done");
    Ok(())
}

// ============================================================
// 工具
// ============================================================

/// 文章 URL slug：优先使用 slug，否则用 id
pub(crate) fn post_slug_or_id(post: &Post) -> &str {
    if post.slug.is_empty() {
        &post.id
    } else {
        &post.slug
    }
}

/// 临时内联 slugify（避免引入额外 crate）
pub(crate) fn slugify(text: &str) -> String {
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

// ============================================================
// 单元测试（纯函数，不依赖 Tauri）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::post::{Post, SiteConfig};
    use context::{ListItem, SiteContext, build_post_context};

    fn sample_post() -> Post {
        Post {
            id: "abc-123".into(),
            title: "你好 World!".into(),
            slug: "hello-world".into(),
            content: "# 标题\n\n> 引用段落\n\n一些正文文字。".into(),
            status: "published".into(),
            tags: vec!["rust".into(), "测试".into()],
            category: "技术".into(),
            cover: "assets/cover.png".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
        }
    }

    fn sample_site_context() -> SiteContext {
        let cfg = SiteConfig {
            title: "Test Blog".into(),
            author: "Tester".into(),
            url: "https://example.com".into(),
            ..Default::default()
        };
        SiteContext::new(&cfg)
    }

    // --------------------------------------------------------
    // slugify
    // --------------------------------------------------------

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Foo--Bar  "), "foo--bar");
        // Unicode 字母被 is_alphanumeric 保留
        assert_eq!(slugify("你好 World!"), "你好-world");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("a!@#$b"), "ab");
        assert_eq!(slugify("---abc---"), "abc"); // trim_matches('-')
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_only_special_chars_returns_empty() {
        assert_eq!(slugify("!!!???"), "");
    }

    // --------------------------------------------------------
    // post_slug_or_id + PostContext
    // --------------------------------------------------------

    #[test]
    fn post_slug_or_id_prefers_slug() {
        let mut p = sample_post();
        assert_eq!(post_slug_or_id(&p), "hello-world");
        p.slug = String::new();
        assert_eq!(post_slug_or_id(&p), "abc-123");
    }

    #[test]
    fn build_post_context_url() {
        let site = sample_site_context();
        let p = sample_post();
        let ctx = build_post_context(&site, &p, "<h1>你好 World!</h1>");
        assert_eq!(ctx.post.url, "/posts/hello-world.html");
        assert_eq!(ctx.post.title, "你好 World!");
        assert_eq!(ctx.post.content, "<h1>你好 World!</h1>");
    }

    #[test]
    fn build_post_context_empty_slug_falls_back_to_id() {
        let site = sample_site_context();
        let mut p = sample_post();
        p.slug = String::new();
        let ctx = build_post_context(&site, &p, "");
        assert_eq!(ctx.post.url, "/posts/abc-123.html");
    }

    // --------------------------------------------------------
    // ListItem::from_post
    // --------------------------------------------------------

    #[test]
    fn list_item_empty_slug_uses_id() {
        let mut p = sample_post();
        p.slug = String::new();
        let item = ListItem::from_post(&p);
        assert_eq!(item.url, "/posts/abc-123.html");
        assert_eq!(item.slug, "abc-123");
    }

    #[test]
    fn list_item_excerpt_truncates() {
        let mut p = sample_post();
        p.content = "a".repeat(500);
        let item = ListItem::from_post(&p);
        // 按行边界 break，长度略超 200 属正常（每行末尾加空格）
        assert!(item.excerpt.chars().count() >= 200);
        assert!(item.excerpt.len() < 230, "excerpt 不应过长：{}", item.excerpt.len());
    }

    // --------------------------------------------------------
    // render_markdown
    // --------------------------------------------------------

    #[test]
    fn render_markdown_basic_elements() {
        let html = render::render_markdown("# Hello\n\nThis is **bold** text.");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn render_markdown_strikethrough() {
        let html = render::render_markdown("~~gone~~ kept");
        assert!(html.contains("<del>gone</del>"));
    }

    #[test]
    fn render_markdown_tasklist() {
        let html = render::render_markdown("- [x] done\n- [ ] todo");
        assert!(html.contains(r#"checked=""#) || html.contains("checked"));
    }

    #[test]
    fn render_markdown_code_block_has_class_language() {
        let html = render::render_markdown("```rust\nlet x = 1;\n```");
        // syntect 渲染后的 code block 应包含 language-rust class
        assert!(html.contains("language-rust") || html.contains("rust"), "got: {html}");
    }

    // --------------------------------------------------------
    // SiteContext::new
    // --------------------------------------------------------

    #[test]
    fn site_context_copies_cfg_fields() {
        let cfg = SiteConfig {
            title: "T".into(),
            description: "D".into(),
            author: "A".into(),
            url: "https://x.com".into(),
            theme: "dark".into(),
            posts_per_page: 42,
            icp: "京ICP备1号".into(),
            ..Default::default()
        };
        let ctx = SiteContext::new(&cfg);
        assert_eq!(ctx.title, "T");
        assert_eq!(ctx.theme, "dark");
        assert_eq!(ctx.posts_per_page, 42);
        assert_eq!(ctx.icp, "京ICP备1号");
        // year 被填充
        assert!(ctx.year >= 2024);
    }
}

// ============================================================
// 集成测试（真 generate：临时 DB + 临时主题 + 断言产物）
// ============================================================
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::db::DbState;
    use crate::error::AppError;
    use crate::models::post::SiteConfig;
    use tempfile::TempDir;

    fn prepare_db() -> DbState {
        let db = DbState(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().expect("open in-memory db"),
        ));
        let conn = db.0.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE posts (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                slug TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'draft',
                tags TEXT NOT NULL DEFAULT '[]',
                category TEXT NOT NULL DEFAULT '',
                cover TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE site_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();

        let cfg = SiteConfig {
            title: "Integration Blog".into(),
            author: "CI Tester".into(),
            url: "https://blog.example.com".into(),
            posts_per_page: 10,
            theme: "minimal".into(),
            ..Default::default()
        };
        conn.execute(
            "INSERT INTO site_config (key, value) VALUES (?1, ?2)",
            rusqlite::params!["main", toml::to_string(&cfg).unwrap()],
        )
        .unwrap();

        let now = "2026-06-01T00:00:00Z";
        conn.execute(
            "INSERT INTO posts (id,title,slug,content,status,tags,category,cover,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                "post-1", "First Post", "first-post", "# Hello\n\n**bold** body",
                "published", r#"["rust","web"]"#, "Tech", "", now, now
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO posts (id,title,slug,content,status,tags,category,cover,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                "post-2", "Second (draft)", "second-draft", "# Draft",
                "draft", "[]", "", "", now, now
            ],
        )
        .unwrap();
        drop(conn);
        db
    }

    /// 最小主题模板集（覆盖 generate 里所有 render_template 调用）
    fn prepare_theme(dir: &std::path::Path) {
        let tpl = dir;
        let assets = dir.join("assets");
        std::fs::create_dir_all(&assets).unwrap();

        // post.tera 上下文 PostContext { site, post }（post 含 title / content | safe / url）
        std::fs::write(
            tpl.join("post.tera"),
            "<html><head><title>{{ site.title }} - {{ post.title }}</title></head>\
             <body><h1>{{ post.title }}</h1><div>{{ post.content | safe }}</div></body></html>",
        )
        .unwrap();
        // index.tera 上下文 IndexContext { site, posts, page, total_pages, prev_url, next_url }
        std::fs::write(
            tpl.join("index.tera"),
            "<html><body><h1>{{ site.title }}</h1>\
             {% for i in posts %}<a href=\"{{ i.url }}\">{{ i.title }}</a>{% endfor %}</body></html>",
        )
        .unwrap();
        // archive.tera 上下文 ArchiveContext { site, groups: Vec<(String, Vec<ListItem>)> }
        // Tera 不支持直接解构 `for (y, items) in groups`，需要先 iterate 再访问
        std::fs::write(
            tpl.join("archive.tera"),
            "<html><body><h1>Archive</h1>\
             {% for g in groups %}<section>{{ g.0 }}</section>{% endfor %}</body></html>",
        )
        .unwrap();
        // tag.tera 上下文 TagContext { site, tag, posts }
        std::fs::write(
            tpl.join("tag.tera"),
            "<html><body><h1>#{{ tag }}</h1>\
             {% for i in posts %}<a href=\"{{ i.url }}\">{{ i.title }}</a>{% endfor %}</body></html>",
        )
        .unwrap();
        // category.tera 上下文 CategoryContext { site, category, posts }
        std::fs::write(
            tpl.join("category.tera"),
            "<html><body><h1>{{ category }}</h1>\
             {% for i in posts %}<a href=\"{{ i.url }}\">{{ i.title }}</a>{% endfor %}</body></html>",
        )
        .unwrap();
        // links.tera 上下文 LinksContext { site }（links 字段可能来自 SiteConfig）
        std::fs::write(
            tpl.join("links.tera"),
            "<html><body><h1>Links</h1></body></html>",
        )
        .unwrap();
        std::fs::write(assets.join("style.css"), "body{}").unwrap();
    }

    #[test]
    fn generate_produces_expected_artifacts() {
        let db = prepare_db();
        let themes_tmp = TempDir::new().unwrap();
        let theme_root = themes_tmp.path();
        let minimal = theme_root.join("minimal");
        std::fs::create_dir_all(&minimal).unwrap();
        prepare_theme(&minimal);

        let user_assets = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let paths = GeneratorPaths {
            themes_dir: theme_root.to_path_buf(),
            user_assets_dir: user_assets.path().to_path_buf(),
        };

        generate(
            &paths,
            &NullProgress,
            &db,
            output.path().to_str().unwrap(),
        )
        .expect("generate must succeed");

        let out = output.path();
        for name in [
            "index.html",
            "archive.html",
            "links.html",
            "sitemap.xml",
            "feed.xml",
            "robots.txt",
        ] {
            assert!(out.join(name).exists(), "{name} 缺失");
        }

        // 主题 assets
        assert!(
            out.join("assets/style.css").exists(),
            "主题 assets 未复制"
        );

        // 已发布文章 HTML
        let first = out.join("posts/first-post.html");
        assert!(first.exists(), "published 文章 HTML 缺失");
        let c = std::fs::read_to_string(&first).unwrap();
        assert!(c.contains("First Post"));
        assert!(c.contains("<strong>bold</strong>"), "Markdown 未渲染");
        assert!(c.contains("Integration Blog"));

        // draft 不应出现
        assert!(
            !out.join("posts/second-draft.html").exists(),
            "draft 不应生成"
        );

        // 标签页（rust, web）和分类页（Tech）
        assert!(out.join("tags/rust.html").exists(), "tag rust 缺失");
        assert!(out.join("tags/web.html").exists(), "tag web 缺失");
        assert!(
            out.join("categories/tech.html").exists(),
            "category 缺失"
        );
    }

    #[test]
    fn slug_duplicate_is_rejected() {
        let db = DbState(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        ));
        {
            let conn = db.0.lock().unwrap();
            conn.execute_batch(
                "CREATE TABLE posts (
                    id TEXT PRIMARY KEY, title TEXT NOT NULL, slug TEXT NOT NULL DEFAULT '',
                    content TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'draft',
                    tags TEXT NOT NULL DEFAULT '[]', category TEXT NOT NULL DEFAULT '',
                    cover TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                CREATE TABLE site_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
            let cfg = SiteConfig {
                title: "T".into(),
                author: "A".into(),
                url: "https://u".into(),
                theme: "minimal".into(),
                ..Default::default()
            };
            conn.execute(
                "INSERT INTO site_config (key, value) VALUES (?1, ?2)",
                rusqlite::params!["main", toml::to_string(&cfg).unwrap()],
            )
            .unwrap();
            let now = "2026-06-01T00:00:00Z";
            for id in ["p1", "p2"] {
                conn.execute(
                    "INSERT INTO posts (id,title,slug,content,status,tags,category,cover,created_at,updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    rusqlite::params![id, "Same", "dup", "# x", "published", "[]", "", "", now, now],
                )
                .unwrap();
            }
        }

        let themes = TempDir::new().unwrap();
        let t = themes.path().join("minimal");
        std::fs::create_dir_all(&t).unwrap();
        prepare_theme(&t);
        let output = TempDir::new().unwrap();
        let paths = GeneratorPaths {
            themes_dir: themes.path().to_path_buf(),
            user_assets_dir: TempDir::new().unwrap().path().to_path_buf(),
        };

        let err = generate(
            &paths,
            &NullProgress,
            &db,
            output.path().to_str().unwrap(),
        )
        .expect_err("duplicate slug must fail");
        assert!(
            matches!(err, AppError::Validation(_)),
            "应为 Validation，实际: {err:?}"
        );
    }

    // --------------------------------------------------------
    // 增量生成：删文 → 重新 generate → 旧 HTML 应被清理
    // --------------------------------------------------------

    #[test]
    fn delete_then_generate_cleans_stale_html() {
        let db = DbState(std::sync::Mutex::new(
            rusqlite::Connection::open_in_memory().unwrap(),
        ));
        {
            let conn = db.0.lock().unwrap();
            conn.execute_batch(
                "CREATE TABLE posts (
                    id TEXT PRIMARY KEY, title TEXT NOT NULL, slug TEXT NOT NULL DEFAULT '',
                    content TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'draft',
                    tags TEXT NOT NULL DEFAULT '[]', category TEXT NOT NULL DEFAULT '',
                    cover TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                );
                CREATE TABLE site_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .unwrap();
            let cfg = SiteConfig {
                title: "T".into(), author: "A".into(), url: "https://u".into(),
                theme: "minimal".into(), ..Default::default()
            };
            conn.execute(
                "INSERT INTO site_config (key, value) VALUES (?1, ?2)",
                rusqlite::params!["main", toml::to_string(&cfg).unwrap()],
            )
            .unwrap();
            let now = "2026-06-01T00:00:00Z";
            let rows = [
                ("p1", "A", "a", "# A"),
                ("p2", "B", "b", "# B"),
                ("p3", "C", "c", "# C"),
            ];
            for (id, title, slug, content) in rows {
                conn.execute(
                    "INSERT INTO posts (id,title,slug,content,status,tags,category,cover,created_at,updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    rusqlite::params![id, title, slug, content, "published", "[]", "", "", now, now],
                )
                .unwrap();
            }
        }

        let themes = TempDir::new().unwrap();
        let t = themes.path().join("minimal");
        std::fs::create_dir_all(&t).unwrap();
        prepare_theme(&t);
        let output = TempDir::new().unwrap();
        let paths = GeneratorPaths {
            themes_dir: themes.path().to_path_buf(),
            user_assets_dir: TempDir::new().unwrap().path().to_path_buf(),
        };

        // 第一次 generate
        generate(&paths, &NullProgress, &db, output.path().to_str().unwrap()).unwrap();
        let out = output.path();
        assert!(out.join("posts/a.html").exists());
        assert!(out.join("posts/b.html").exists());
        assert!(out.join("posts/c.html").exists());
        let sitemap = std::fs::read_to_string(out.join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("/posts/b.html"));

        // 删 p2
        {
            let conn = db.0.lock().unwrap();
            conn.execute("DELETE FROM posts WHERE id = 'p2'", []).unwrap();
        }

        // 第二次 generate
        generate(&paths, &NullProgress, &db, output.path().to_str().unwrap()).unwrap();

        // 旧 HTML 必须消失
        assert!(
            !out.join("posts/b.html").exists(),
            "删除的文章 b.html 应被清理"
        );
        // 其他两篇继续存在
        assert!(out.join("posts/a.html").exists());
        assert!(out.join("posts/c.html").exists());

        // sitemap 也必须更新（不再含 b）
        let sitemap2 = std::fs::read_to_string(out.join("sitemap.xml")).unwrap();
        assert!(!sitemap2.contains("/posts/b.html"), "sitemap 应移除 b");
        assert!(sitemap2.contains("/posts/a.html"));
    }

    // --------------------------------------------------------
    // 增量状态文件生成 & 再生成
    // --------------------------------------------------------

    #[test]
    fn state_file_written_after_generate() {
        let themes = TempDir::new().unwrap();
        let t = themes.path().join("minimal");
        std::fs::create_dir_all(&t).unwrap();
        prepare_theme(&t);

        let db = prepare_db();
        let output = TempDir::new().unwrap();
        let paths = GeneratorPaths {
            themes_dir: themes.path().to_path_buf(),
            user_assets_dir: TempDir::new().unwrap().path().to_path_buf(),
        };

        generate(&paths, &NullProgress, &db, output.path().to_str().unwrap()).unwrap();

        let state_path = output.path().join(".gen_state.json");
        assert!(state_path.exists(), ".gen_state.json 必须存在");
        let state_json = std::fs::read_to_string(&state_path).unwrap();
        // 应有 theme + 1 个 published post 的哈希（draft 不进 state）
        assert!(
            state_json.contains("minimal"),
            "state 应记录主题名"
        );
        assert!(
            state_json.contains("post-1"),
            "state 应记录 published post id"
        );
        assert!(
            !state_json.contains("post-2"),
            "state 不应包含 draft post id"
        );

        // 第二次 generate（文章内容未变），state 文件应被更新
        generate(&paths, &NullProgress, &db, output.path().to_str().unwrap()).unwrap();
        let state_json2 = std::fs::read_to_string(&state_path).unwrap();
        assert_eq!(
            state_json, state_json2,
            "内容未变 → state 不应变化"
        );
    }
}
