//! 文章相关 Tauri commands
//!
//! 业务 SQL 已收进 `db::posts` 仓储，commands 层只做：
//! - 参数校验（id 格式、文件存在性、图片类型白名单）
//! - AppError → String 转换（Tauri command 返回契约）
//! - 删除时同步清理资源目录

use crate::db;
use crate::error::AppError;
use crate::models::post::{Post, PostMeta};
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
pub fn delete_post(
    app: AppHandle,
    db: State<'_, db::DbState>,
    id: String,
) -> Result<(), String> {
    validate_post_id(&id)?;
    // 文章不存在时不报错（幂等）
    let deleted = db::posts::delete(&db, &id)
        .map_err(|e: AppError| e.to_frontend())?;
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
pub fn import_image(
    app: AppHandle,
    post_id: String,
    src_path: String,
) -> Result<String, String> {
    validate_post_id(&post_id)?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Tauri(e.to_string()).to_frontend())?;
    let src = std::path::Path::new(&src_path);
    if !src.is_file() {
        return Err(AppError::Validation(format!("Image file not found: {src_path}")).to_frontend());
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "png".into());
    // 仅允许图片扩展名，避免把任意文件复制进站点资源目录
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"];
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(
            AppError::Validation(format!("Unsupported image type: {ext}")).to_frontend()
        );
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
