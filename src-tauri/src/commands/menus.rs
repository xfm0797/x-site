//! 站点导航菜单命令：读取 / 保存
//!
//! 菜单保存在 SiteConfig.menus 中，生成站点时注入模板 `site.menus` 渲染。

use crate::db;
use crate::db::DbState;
use crate::error::AppError;
use crate::models::post::MenuItem;
use tauri::{AppHandle, Emitter, State};

/// 获取站点导航菜单
#[tauri::command]
pub fn get_menus(db: State<'_, DbState>) -> Result<Vec<MenuItem>, String> {
    let cfg = db::config::load(&db).map_err(|e| e.to_frontend())?;
    Ok(cfg.menus)
}

/// 保存站点导航菜单
///
/// 只更新 SiteConfig.menus，避免与通用设置面板的整份配置互相覆盖。
#[tauri::command]
pub fn save_menus(
    app: AppHandle,
    db: State<'_, DbState>,
    menus: Vec<MenuItem>,
) -> Result<(), String> {
    if menus.len() > 50 {
        return Err(AppError::Validation("Too many menu items (max 50)".into()).to_frontend());
    }
    if menus
        .iter()
        .any(|m| m.label.trim().is_empty() || m.url.trim().is_empty())
    {
        return Err(
            AppError::Validation("Menu label and url must not be empty".into()).to_frontend(),
        );
    }
    let mut cfg = db::config::load(&db).map_err(|e| e.to_frontend())?;
    cfg.menus = menus;
    db::config::save(&db, &cfg).map_err(|e| e.to_frontend())?;
    let _ = app.emit("menus://changed", ());
    Ok(())
}
