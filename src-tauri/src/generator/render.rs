//! Markdown → HTML 渲染 + Tera 模板加载与渲染

use super::context::{
    ArchiveContext, CategoryContext, IndexContext, LinksContext, ListItem,
    SiteContext, TagContext,
};
use crate::models::post::Post;
use pulldown_cmark::{html, Event, Options, Parser, Tag};
use std::fs;
use std::path::Path;
use syntect::highlighting::ThemeSet;
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tera::{Context, Tera};

// ============================================================
// 主题模板加载
// ============================================================

/// 从主题目录加载所有 .tera 文件为一个 Tera 实例
///
/// 目录结构约定：
///   themes/<name>/
///     post.tera
///     index.tera
///     archive.tera
///     tag.tera
///     category.tera
///     links.tera
///     partials/      (可选 partial)
///       header.tera
///       footer.tera
///     assets/
pub fn load_theme(theme_dir: &Path) -> Result<Tera, String> {
    if !theme_dir.exists() {
        // 兜底：使用内嵌默认主题（来自内置 themes/default/）
        return load_embedded_theme();
    }

    // Tera 要求以 glob 方式注册模板；这里把目录里所有 .tera 都加进去
    let mut tera = Tera::default();
    let mut entries = vec![];
    collect_tera_files(theme_dir, &mut entries)?;

    if entries.is_empty() {
        return load_embedded_theme();
    }

    for (name, content) in entries {
        tera.add_raw_template(&name, &content)
            .map_err(|e| format!("tera add template {name}: {e}"))?;
    }

    tera.autoescape_on(vec!["html", "tera"]);
    Ok(tera)
}

/// 递归收集所有 .tera 文件
/// name 是相对路径（去掉扩展名），如 "partials/header"
fn collect_tera_files(
    dir: &Path,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_tera_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("tera") {
            let rel = path
                .strip_prefix(dir.parent().unwrap_or(Path::new("")))
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            // 简化：直接用文件名（不含目录）作为模板名
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let content =
                std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            out.push((name, content.clone()));
            // 也注册带相对路径的版本（用于 partials）
            if rel.contains('/') {
                out.push((rel, content));
            }
        }
    }
    Ok(())
}

/// 加载内嵌默认主题（themes/default/ 已通过 include_str! 嵌入二进制）
fn load_embedded_theme() -> Result<Tera, String> {
    let mut tera = Tera::default();
    let templates = [
        ("post", include_str!("../../themes/default/post.tera")),
        ("index", include_str!("../../themes/default/index.tera")),
        ("archive", include_str!("../../themes/default/archive.tera")),
        ("tag", include_str!("../../themes/default/tag.tera")),
        ("category", include_str!("../../themes/default/category.tera")),
        ("links", include_str!("../../themes/default/links.tera")),
        ("base", include_str!("../../themes/default/base.tera")),
    ];
    for (name, content) in templates {
        tera
            .add_raw_template(&format!("{name}.tera"), content)
            .map_err(|e| format!("load embedded template {name}: {e}"))?;
    }
    tera.autoescape_on(vec!["html", "tera"]);
    Ok(tera)
}

// ============================================================
// pulldown-cmark + syntect 渲染
// ============================================================

/// 单例 SyntaxSet（线程安全懒加载）
/// 用 once_cell 避免每次渲染重复加载语法库
static SYNTAX_SET: once_cell::sync::Lazy<SyntaxSet> =
    once_cell::sync::Lazy::new(SyntaxSet::load_defaults_newlines);

/// 单例高亮主题
static THEME: once_cell::sync::Lazy<syntect::highlighting::Theme> =
    once_cell::sync::Lazy::new(|| {
        ThemeSet::load_defaults()
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap()
    });

/// 把 Markdown 字符串渲染为 HTML
///
/// - 行号、表格、任务列表、删除线、脚注等扩展都启用
/// - 代码块用 syntect 高亮，输出带内联样式的 HTML（避免前端额外加载 CSS）
pub fn render_markdown(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(markdown, options);

    // 为代码块做高亮：拦截 Event::* 中的 CodeBlock，
    // 用 syntect 把代码渲染成带样式的 HTML 字符串，
    // 然后作为 Html 事件返回（pulldown-cmark 会原样输出）
    let processed = parser.map(|event| match event {
        Event::Start(Tag::CodeBlock(ref _kind)) => {
            // pulldown-cmark 不在 Start 阶段给内容，统一在 highlight_code_blocks 后处理
            event
        }
        _ => event,
    });

    // 简单实现：直接用 pulldown-cmark 的 html::push_html，
    // 再用后处理 regex 给 <pre><code class="language-xxx"> 加高亮。
    // 这样实现简单且能覆盖大多数场景。
    let mut html_output = String::new();
    html::push_html(&mut html_output, processed);

    // 后处理：对每个 ``` 代码块用 syntect 渲染
    highlight_code_blocks(&html_output)
}

/// 把 HTML 中 `<pre><code class="language-xxx">...</code></pre>` 用 syntect 重新高亮
fn highlight_code_blocks(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut rest = html;

    loop {
        match rest.find("<pre><code") {
            Some(start) => {
                result.push_str(&rest[..start]);
                let after_start = &rest[start..];
                let end = match after_start.find("</code></pre>") {
                    Some(e) => e + "</code></pre>".len(),
                    None => {
                        // 不闭合，原样追加剩余并退出
                        result.push_str(after_start);
                        break;
                    }
                };
                let block = &after_start[..end];

                // 解析 language 和 code 内容
                let (lang, code) = parse_code_block(block);
                let highlighted = if let Some(syntax) = find_syntax(&lang) {
                    syntect::html::highlighted_html_for_string(
                        &code,
                        &SYNTAX_SET,
                        syntax,
                        &THEME,
                    )
                    .unwrap_or_else(|_| html_escape(&code))
                } else {
                    format!("<pre><code class=\"language-{}\">{}</code></pre>", lang, html_escape(&code))
                };
                result.push_str(&highlighted);
                rest = &after_start[end..];
            }
            None => {
                result.push_str(rest);
                break;
            }
        }
    }
    result
}

/// 从 `<pre><code class="language-rust">code</code></pre>` 解析出 (lang, code)
fn parse_code_block(block: &str) -> (String, String) {
    // 找到第一个 `>` 后取内容直到 `</code></pre>`
    let first_gt = block.find('>').unwrap_or(0);
    let after_class = &block[first_gt + 1..];
    let end_tag = "</code></pre>";
    let content_end = after_class
        .find(end_tag)
        .map(|i| i)
        .unwrap_or(after_class.len());
    let inner = &after_class[..content_end];

    // 从 block 解析 language
    let lang = if let Some(start) = block.find("language-") {
        let after = &block[start + "language-".len()..];
        let end = after.find(|c: char| c == '"' || c == ' ').unwrap_or(after.len());
        after[..end].to_string()
    } else {
        String::new()
    };

    // 去除 HTML 实体
    let code = html_unescape(inner);
    (lang, code)
}

/// 根据 language 名字找到 SyntaxReference
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    if lang.is_empty() {
        return SYNTAX_SET.find_syntax_by_token("rust");
    }
    SYNTAX_SET
        .find_syntax_by_token(lang)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(lang))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

// ============================================================
// Tera 模板渲染入口
// ============================================================

pub fn render_template<T: serde::Serialize>(
    tera: &Tera,
    template: &str,
    ctx: &T,
) -> Result<String, String> {
    let context = Context::from_serialize(ctx)
        .map_err(|e| format!("build context for {template}: {e}"))?;
    tera.render(template, &context)
        .map_err(|e| format!("render {template}: {e}"))
}

// ============================================================
// 各类型页面渲染
// ============================================================

pub fn render_index_page(
    tera: &Tera,
    site: &SiteContext,
    posts: &[Post],
    page: usize,
    total_pages: usize,
) -> Result<String, String> {
    let list: Vec<ListItem> = posts.iter().map(ListItem::from_post).collect();
    let prev_url = if page > 1 {
        Some(if page == 2 {
            "/".to_string()
        } else {
            format!("/page/{}.html", page - 1)
        })
    } else {
        None
    };
    let next_url = if page < total_pages {
        Some(format!("/page/{}.html", page + 1))
    } else {
        None
    };
    let ctx = IndexContext {
        site,
        posts: list,
        page,
        total_pages,
        prev_url,
        next_url,
    };
    render_template(tera, "index.tera", &ctx)
}

pub fn render_archive(
    tera: &Tera,
    site: &SiteContext,
    posts: &[Post],
) -> Result<String, String> {
    // 按年份分组
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<ListItem>> = BTreeMap::new();
    for p in posts {
        // 从 ISO 时间取年份（前 4 位）
        let year = p.created_at.chars().take(4).collect::<String>();
        groups
            .entry(year)
            .or_default()
            .push(ListItem::from_post(p));
    }
    // BTreeMap 自动按 key 排序，但我们要年份降序
    let mut groups: Vec<(String, Vec<ListItem>)> = groups.into_iter().collect();
    groups.reverse();
    let ctx = ArchiveContext { site, groups };
    render_template(tera, "archive.tera", &ctx)
}

pub fn render_tag_page(
    tera: &Tera,
    site: &SiteContext,
    tag: &str,
    posts: &[&Post],
) -> Result<String, String> {
    let list: Vec<ListItem> = posts.iter().map(|p| ListItem::from_post(p)).collect();
    let ctx = TagContext {
        site,
        tag: tag.to_string(),
        posts: list,
    };
    render_template(tera, "tag.tera", &ctx)
}

pub fn render_category_page(
    tera: &Tera,
    site: &SiteContext,
    category: &str,
    posts: &[&Post],
) -> Result<String, String> {
    let list: Vec<ListItem> = posts.iter().map(|p| ListItem::from_post(p)).collect();
    let ctx = CategoryContext {
        site,
        category: category.to_string(),
        posts: list,
    };
    render_template(tera, "category.tera", &ctx)
}

pub fn render_links_page(
    tera: &Tera,
    site: &SiteContext,
) -> Result<String, String> {
    let ctx = LinksContext { site };
    render_template(tera, "links.tera", &ctx)
}
