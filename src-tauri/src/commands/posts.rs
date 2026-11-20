//! 文章相关 Tauri commands
//!
//! 业务 SQL 已收进 `db::posts` 仓储，commands 层只做：
//! - 参数校验（id 格式、文件存在性、图片类型白名单）
//! - AppError → String 转换（Tauri command 返回契约）
//! - 删除时同步清理资源目录

use crate::db;
use crate::error::AppError;
use crate::models::post::{Post, PostMeta};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Manager, State};

// ============================================================
// 列表 / 详情
// ============================================================

/// 加载所有文章元数据（不含正文），供首页列表使用
#[tauri::command]
pub fn load_posts(db: State<'_, db::DbState>) -> Result<Vec<PostMeta>, String> {
    db::posts::load_all_meta(&db).map_err(|e| e.to_frontend())
}

/// 按 id 加载完整文章（含正文），编辑器打开已有文章时使用
#[tauri::command]
pub fn load_post(db: State<'_, db::DbState>, id: String) -> Result<Post, String> {
    db::posts::load_by_id(&db, &id).map_err(|e| e.to_frontend())
}

/// 删除文章，并清理其资源目录（assets/<post_id>/）
#[tauri::command]
pub fn delete_post(app: AppHandle, db: State<'_, db::DbState>, id: String) -> Result<(), String> {
    validate_post_id(&id)?;
    // 文章不存在时不报错（幂等）
    let deleted = db::posts::delete(&db, &id).map_err(|e: AppError| e.to_frontend())?;
    if !deleted {
        return Ok(());
    }
    // 资源目录清理失败不阻断删除
    if let Ok(dir) = app.path().app_data_dir() {
        let assets = dir.join("assets").join(&id);
        if assets.exists() {
            let _ = std::fs::remove_dir_all(&assets);
        }
    }
    Ok(())
}

/// 文章 id 仅允许字母数字、连字符、下划线，防止路径遍历
fn validate_post_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AppError::Validation(format!("Invalid post id: {id}")).to_frontend())
    }
}

// ============================================================
// 保存
// ============================================================

/// 保存文章（新增或更新，基于 id 做 UPSERT）
#[tauri::command]
pub fn save_post(db: State<'_, db::DbState>, post: Post) -> Result<Post, String> {
    db::posts::save(&db, &post).map_err(|e| e.to_frontend())
}

// ============================================================
// 图片导入
// ============================================================

/// 把 src_path 指定的图片复制到 assets/<post_id>/ 下，返回可在 Markdown 引用的相对路径
#[tauri::command]
pub fn import_image(app: AppHandle, post_id: String, src_path: String) -> Result<String, String> {
    validate_post_id(&post_id)?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Tauri(e.to_string()).to_frontend())?;
    let src = std::path::Path::new(&src_path);
    if !src.is_file() {
        return Err(
            AppError::Validation(format!("Image file not found: {src_path}")).to_frontend(),
        );
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".into());
    // 仅允许图片扩展名，避免把任意文件复制进站点资源目录
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"];
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(AppError::Validation(format!("Unsupported image type: {ext}")).to_frontend());
    }

    // 生成不重复的文件名：时间戳 + 短随机
    let ts = chrono::Utc::now().timestamp_millis();
    let rand: u32 = rand::random();
    let filename = format!("img-{ts}-{rand:x}.{ext}");

    let assets_dir = app_data_dir.join("assets").join(&post_id);
    std::fs::create_dir_all(&assets_dir).map_err(|e| AppError::Io(e).to_frontend())?;

    let target = assets_dir.join(&filename);
    std::fs::copy(src, &target).map_err(|e| AppError::Io(e).to_frontend())?;

    Ok(format!("assets/{post_id}/{filename}"))
}

// ============================================================
// Markdown 批量导入
// ============================================================

/// 导入结果汇总
#[derive(Debug, Serialize)]
pub struct ImportSummary {
    /// 成功导入数量
    pub imported: usize,
    /// 被跳过的文件（非 Markdown 或读取失败）
    pub skipped: Vec<String>,
    /// 新导入文章元数据，供前端直接刷新列表
    pub posts: Vec<PostMeta>,
}

/// 从本地 Markdown 文件批量导入为草稿文章
#[tauri::command]
pub fn import_markdown(
    db: State<'_, db::DbState>,
    paths: Vec<String>,
) -> Result<ImportSummary, String> {
    // 现有 slug 集合，保证新导入文章不与站内已有 slug 冲突
    let existing = db::posts::load_all_meta(&db).map_err(|e| e.to_frontend())?;
    let mut used_slugs: HashSet<String> = existing.into_iter().map(|m| m.slug).collect();

    let mut summary = ImportSummary {
        imported: 0,
        skipped: Vec::new(),
        posts: Vec::new(),
    };

    for path in paths {
        let src = std::path::Path::new(&path);
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        if !matches!(ext.as_str(), "md" | "markdown") {
            summary.skipped.push(path);
            continue;
        }
        let raw = match std::fs::read_to_string(src) {
            Ok(s) => s,
            Err(_) => {
                summary.skipped.push(path);
                continue;
            }
        };

        let fallback_title = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let parsed = parse_markdown(&raw, &fallback_title);
        let slug = unique_slug(&parsed.slug, &mut used_slugs);

        let now = chrono::Utc::now().to_rfc3339();
        let post = Post {
            id: new_post_id(),
            title: parsed.title,
            slug,
            content: parsed.content,
            status: parsed.status,
            tags: parsed.tags,
            category: parsed.category,
            cover: String::new(),
            created_at: parsed.created_at.unwrap_or_else(|| now.clone()),
            updated_at: now,
        };
        let saved = db::posts::save(&db, &post).map_err(|e| e.to_frontend())?;
        summary.posts.push(PostMeta {
            id: saved.id,
            title: saved.title,
            slug: saved.slug,
            status: saved.status,
            tags: saved.tags,
            category: saved.category,
            cover: saved.cover,
            created_at: saved.created_at,
            updated_at: saved.updated_at,
        });
        summary.imported += 1;
    }

    Ok(summary)
}

/// 生成符合 validate_post_id 约束的文章 id（字母数字 + 连字符）
fn new_post_id() -> String {
    let ts = chrono::Utc::now().timestamp_millis();
    let rand: u32 = rand::random();
    format!("post-{ts}-{rand:x}")
}

/// 解析后的 Markdown 文章字段
struct ParsedMarkdown {
    title: String,
    slug: String,
    content: String,
    status: String,
    tags: Vec<String>,
    category: String,
    created_at: Option<String>,
}

/// 解析 Markdown：可选 frontmatter + 正文
///
/// frontmatter 支持键：title / slug / date / category / tags / status。
/// 无 title 时取正文首个 `# 标题`（并从正文移除），再退化为文件名。
fn parse_markdown(raw: &str, fallback_title: &str) -> ParsedMarkdown {
    let (front, body) = split_frontmatter(raw);
    let mut content = body.to_string();

    let mut title = front.get("title").cloned().filter(|s| !s.trim().is_empty());
    if title.is_none() {
        if let Some((heading, rest)) = take_first_h1(&content) {
            title = Some(heading);
            content = rest;
        }
    }
    let title = title.unwrap_or_else(|| fallback_title.to_string());

    let slug = front
        .get("slug")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slugify(&title));

    let status = match front.get("status").map(|s| s.trim().to_lowercase()) {
        Some(s) if s == "published" => "published".to_string(),
        _ => "draft".to_string(),
    };

    ParsedMarkdown {
        title,
        slug,
        content: content.trim_start_matches('\n').to_string(),
        status,
        tags: front.get("tags").map(|t| parse_tags(t)).unwrap_or_default(),
        category: front
            .get("category")
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        created_at: front.get("date").and_then(|d| normalize_date(d)),
    }
}

/// 分离 `---` 包裹的 frontmatter，返回键值对与正文
fn split_frontmatter(raw: &str) -> (HashMap<String, String>, &str) {
    let Some(after_open) = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
    else {
        return (HashMap::new(), raw);
    };
    let mut offset = 0;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.trim() == "---" {
            let front = parse_frontmatter_kv(&after_open[..offset]);
            let body = &after_open[offset + line.len()..];
            return (front, body);
        }
        offset += line.len();
    }
    // 未闭合视为无 frontmatter
    (HashMap::new(), raw)
}

/// 解析简单 `key: value` 形式的 frontmatter
fn parse_frontmatter_kv(front: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in front.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let mut value = value.trim().to_string();
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            value = value[1..value.len() - 1].to_string();
        }
        map.insert(key, value);
    }
    map
}

/// 取正文首个 `# 标题`，返回标题与移除该行后的正文
fn take_first_h1(body: &str) -> Option<(String, String)> {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(text) = trimmed.strip_prefix("# ") {
            let title = text.trim().to_string();
            if !title.is_empty() {
                let rest = &body[offset + line.len()..];
                return Some((title, rest.to_string()));
            }
        }
        offset += line.len();
    }
    None
}

/// tags 支持 `[a, b]` 或逗号分隔
fn parse_tags(raw: &str) -> Vec<String> {
    let inner = raw
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(raw.trim());
    inner
        .split(',')
        .map(|t| t.trim().trim_matches(['"', '\'']).trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// 由标题生成 slug：保留字母数字（含非 ASCII），其他字符折叠为连字符
fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in text.trim().chars() {
        if c.is_alphanumeric() {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let truncated: String = out.trim_matches('-').chars().take(60).collect();
    truncated.trim_end_matches('-').to_string()
}

/// 生成站内唯一 slug，冲突时追加 -2、-3
fn unique_slug(base: &str, used: &mut HashSet<String>) -> String {
    let base = if base.is_empty() { "post" } else { base };
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// 把常见日期写法归一化为 RFC3339（用于 created_at）
fn normalize_date(raw: &str) -> Option<String> {
    use chrono::TimeZone;
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc).to_rfc3339());
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some(chrono::Utc.from_utc_datetime(&naive).to_rfc3339());
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0)?;
        return Some(chrono::Utc.from_utc_datetime(&naive).to_rfc3339());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_fields() {
        let raw = "---\ntitle: \"Hello World\"\nslug: hello\ncategory: 技术\ntags: [Rust, Tauri]\nstatus: published\ndate: 2024-01-02\n---\n正文内容\n";
        let parsed = parse_markdown(raw, "fallback");
        assert_eq!(parsed.title, "Hello World");
        assert_eq!(parsed.slug, "hello");
        assert_eq!(parsed.category, "技术");
        assert_eq!(parsed.tags, vec!["Rust", "Tauri"]);
        assert_eq!(parsed.status, "published");
        assert_eq!(parsed.content, "正文内容\n");
        assert_eq!(
            parsed.created_at.as_deref(),
            Some("2024-01-02T00:00:00+00:00")
        );
    }

    #[test]
    fn falls_back_to_first_h1_and_removes_it() {
        let raw = "前言\n\n# 我的标题\n\n正文\n";
        let parsed = parse_markdown(raw, "fallback");
        assert_eq!(parsed.title, "我的标题");
        assert_eq!(parsed.slug, "我的标题");
        assert_eq!(parsed.status, "draft");
        assert_eq!(parsed.content, "正文\n");
        assert_eq!(parsed.created_at, None);
    }

    #[test]
    fn falls_back_to_filename_without_heading() {
        let parsed = parse_markdown("只有正文", "my-file");
        assert_eq!(parsed.title, "my-file");
        assert_eq!(parsed.content, "只有正文");
    }

    #[test]
    fn handles_comma_separated_tags() {
        assert_eq!(parse_tags("a, b ,\"c\""), vec!["a", "b", "c"]);
        assert!(parse_tags("[]").is_empty());
    }

    #[test]
    fn slugify_collapses_punctuation() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  Rust & Tauri  "), "rust-tauri");
        assert_eq!(slugify("中文 标题"), "中文-标题");
    }

    #[test]
    fn unique_slug_appends_suffixes() {
        let mut used = HashSet::from(["hello".to_string()]);
        assert_eq!(unique_slug("hello", &mut used), "hello-2");
        assert_eq!(unique_slug("hello", &mut used), "hello-3");
        assert_eq!(unique_slug("other", &mut used), "other");
    }
}
