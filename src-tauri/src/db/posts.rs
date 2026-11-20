//! 文章仓储
//!
//! 集中所有 SQL，消除 commands/posts.rs、generator/mod.rs、commands/theme.rs
//! 里的行映射 / 查询拼装。commands 层只做参数校验和 command 边界适配。

use crate::error::{AppError, AppResult};
use crate::models::post::{Post, PostMeta};
use super::DbState;

/// 加载全部文章元数据（不含正文，减少传输开销）
pub fn load_all_meta(db: &DbState) -> AppResult<Vec<PostMeta>> {
    let conn = db.0.lock()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, slug, status, tags, category, cover, created_at, updated_at
         FROM posts ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_meta)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Db(e.to_string()))
}

/// 按 id 加载完整文章（含正文）
pub fn load_by_id(db: &DbState, id: &str) -> AppResult<Post> {
    let conn = db.0.lock()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, slug, content, status, tags, category, cover,
                created_at, updated_at
         FROM posts WHERE id = ?1",
    )?;
    stmt.query_row(rusqlite::params![id], row_to_post)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::Validation(format!("Post not found: {id}"))
            }
            other => AppError::Db(other.to_string()),
        })
}

/// 加载所有已发布文章（生成静态站点用）
pub fn load_published(db: &DbState) -> AppResult<Vec<Post>> {
    let conn = db.0.lock()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, slug, content, status, tags, category, cover,
                created_at, updated_at
         FROM posts WHERE status = 'published' ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_post)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Db(e.to_string()))
}

/// 保存文章（UPSERT 基于 id）
pub fn save(db: &DbState, post: &Post) -> AppResult<Post> {
    // 用事务包住 update_at 刷新 + 写库，避免两个 writer 交错
    let conn = db.0.lock()?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO posts (id, title, slug, content, status, tags, category, cover, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
             title = excluded.title,
             slug = excluded.slug,
             content = excluded.content,
             status = excluded.status,
             tags = excluded.tags,
             category = excluded.category,
             cover = excluded.cover,
             updated_at = excluded.updated_at",
        rusqlite::params![
            post.id,
            post.title,
            post.slug,
            post.content,
            post.status,
            serde_json::to_string(&post.tags)?,
            post.category,
            post.cover,
            post.created_at,
            now,
        ],
    )?;

    let mut saved = post.clone();
    saved.updated_at = now;
    Ok(saved)
}

/// 删除文章
pub fn delete(db: &DbState, id: &str) -> AppResult<bool> {
    let conn = db.0.lock()?;
    let n = conn.execute("DELETE FROM posts WHERE id = ?1", rusqlite::params![id])?;
    Ok(n > 0)
}

// --- 行映射 ---

fn row_to_post(row: &rusqlite::Row) -> Result<Post, rusqlite::Error> {
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
}

fn row_to_meta(row: &rusqlite::Row) -> Result<PostMeta, rusqlite::Error> {
    let tags_str: String = row.get(4)?;
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
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
}
