use crate::db::DbState;
use crate::models::post::{Post, PostMeta};
use rusqlite::params;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

/// 加载所有文章列表（元数据，不含正文）
#[tauri::command]
pub fn load_posts(db: State<'_, DbState>) -> Result<Vec<PostMeta>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, slug, status, tags, category, cover,
                    created_at, updated_at
             FROM posts ORDER BY datetime(updated_at) DESC",
        )
        .map_err(|e| e.to_string())?;

    let posts = stmt
        .query_map([], |row| {
            let tags_str: String = row.get(4)?;
            let tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(PostMeta {
                id: row.get(0)?,
                title: row.get(1)?,
                slug: row.get(2)?,
                status: row.get(3)?,
                tags,
                category: row.get(5)?,
                cover: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(posts)
}

/// 按 id 加载完整文章（含正文），编辑器打开已有文章时使用
#[tauri::command]
pub fn load_post(db: State<'_, DbState>, id: String) -> Result<Post, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, title, slug, content, status, tags, category, cover,
                created_at, updated_at
         FROM posts WHERE id = ?1",
        params![id],
        |row| {
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
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
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => format!("Post not found: {id}"),
        other => other.to_string(),
    })
}

/// 删除文章，并清理其资源目录（assets/<post_id>/）
#[tauri::command]
pub fn delete_post(app: AppHandle, db: State<'_, DbState>, id: String) -> Result<(), String> {
    validate_post_id(&id)?;
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM posts WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    // 资源目录清理失败不阻断删除（记录即可）
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
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid post id: {id}"))
    }
}

/// 保存文章（新增或更新，基于 id 做 UPSERT）
#[tauri::command]
pub fn save_post(db: State<'_, DbState>, post: Post) -> Result<Post, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let tags_str = serde_json::to_string(&post.tags).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO posts (id, title, slug, content, status, tags,
                            category, cover, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            title = ?2, slug = ?3, content = ?4, status = ?5,
            tags = ?6, category = ?7, cover = ?8, updated_at = ?10",
        params![
            post.id,
            post.title,
            post.slug,
            post.content,
            post.status,
            tags_str,
            post.category,
            post.cover,
            post.created_at,
            post.updated_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(post)
}

/// 导入图片到文章资源目录
///
/// 将外部图片（绝对路径）复制到 `<app_data_dir>/assets/<post_id>/<rand>.<ext>`，
/// 返回可在 Markdown 中引用的相对路径：`assets/<post_id>/<rand>.<ext>`
#[tauri::command]
pub fn import_image(
    app: AppHandle,
    post_id: String,
    src_path: String,
) -> Result<String, String> {
    validate_post_id(&post_id)?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let src = Path::new(&src_path);
    if !src.is_file() {
        return Err(format!("Image file not found: {src_path}"));
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".into());
    // 仅允许图片扩展名，避免把任意文件复制进站点资源目录
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"];
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(format!("Unsupported image type: {ext}"));
    }

    // 生成不重复的文件名：时间戳 + 短随机
    let ts = chrono::Utc::now().timestamp_millis();
    let rand: u32 = rand::random();
    let filename = format!("img-{}-{}.{}", ts, rand, ext);
    let rel_dir = PathBuf::from("assets").join(&post_id);
    let abs_dir = app_data_dir.join(&rel_dir);
    std::fs::create_dir_all(&abs_dir).map_err(|e| e.to_string())?;

    let abs_dst = abs_dir.join(&filename);
    let rel_dst = rel_dir.join(&filename);

    std::fs::copy(src, &abs_dst).map_err(|e| e.to_string())?;

    // 统一返回 Unix 风格的相对路径（用于 Markdown / 前端引用）
    let rel_str = rel_dst.to_string_lossy().replace('\\', "/");
    Ok(rel_str)
}
