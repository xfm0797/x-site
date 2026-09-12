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
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let src = Path::new(&src_path);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".into());

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
