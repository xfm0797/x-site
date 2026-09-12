//! 主题管理命令：列出 / 切换 / 导入

use crate::db::DbState;
use crate::models::post::SiteConfig;
use crate::models::theme::{Theme, ThemeMeta, ThemeState};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager, State};

/// 列出所有已安装主题
///
/// 扫描 themes/ 目录，每个子目录若含 theme.json 则视为有效主题。
/// `active` 字段由当前 SiteConfig.theme 决定。
#[tauri::command]
pub fn list_themes(
    theme_state: State<'_, ThemeState>,
    db: State<'_, DbState>,
) -> Result<Vec<Theme>, String> {
    let active = {
        // 优先从 ThemeState 取（apply_theme 后即时同步）
        let s = theme_state.active.lock().map_err(|e| e.to_string())?;
        s.clone()
    };
    let mut themes: Vec<Theme> = Vec::new();

    if !theme_state.themes_dir.exists() {
        return Ok(themes);
    }

    let entries = fs::read_dir(&theme_state.themes_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("theme.json");
        if !meta_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&meta_path).map_err(|e| e.to_string())?;
        let mut meta: ThemeMeta =
            serde_json::from_str(&content).unwrap_or_default();
        // 目录名作为唯一 name
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        meta.name = dir_name;
        let is_active = meta.name == active;
        themes.push(Theme { meta, active: is_active });
    }

    // 激活的排第一，然后按显示名
    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, _) => std::cmp::Ordering::Less,
        (_, true) => std::cmp::Ordering::Greater,
        _ => a.meta.display_name.cmp(&b.meta.display_name),
    });

    // F: 从 db 拉取 site_config，覆盖 active 标记
    if let Ok(cfg) = load_site_config(&db) {
        for t in themes.iter_mut() {
            t.active = t.meta.name == cfg.theme;
        }
    }

    Ok(themes)
}

/// 切换当前主题
///
/// 更新 SiteConfig.theme 字段并写回数据库，同时更新 ThemeState。
#[tauri::command]
pub fn apply_theme(
    app: AppHandle,
    theme_state: State<'_, ThemeState>,
    db: State<'_, DbState>,
    name: String,
) -> Result<(), String> {
    // 校验主题是否存在
    let theme_dir = theme_state.themes_dir.join(&name);
    if !theme_dir.exists() {
        return Err(format!("Theme '{}' not found", name));
    }

    // 更新 SiteConfig
    let mut cfg = load_site_config(&db)?;
    cfg.theme = name.clone();
    save_site_config(&db, &cfg)?;

    // 同步 ThemeState
    {
        let mut active = theme_state.active.lock().map_err(|e| e.to_string())?;
        *active = name.clone();
    }

    // 通知前端主题已切换
    let _ = app.emit("theme://changed", &name);
    Ok(())
}

/// 从 zip 包导入主题
///
/// 解压到 themes/<包名>/。zip 包根目录需含 theme.json。
/// 如果同名主题已存在，会被覆盖。
#[tauri::command]
pub fn import_theme(
    theme_state: State<'_, ThemeState>,
    zip_path: String,
) -> Result<String, String> {
    let zip_file = Path::new(&zip_path);
    if !zip_file.exists() {
        return Err(format!("File not found: {}", zip_path));
    }

    let file = fs::File::open(zip_file).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    // 探测主题名：取第一个含 theme.json 的目录前缀
    let mut theme_name: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with("/theme.json") || name == "theme.json" {
            // 取 theme.json 的上一级目录名作为主题名
            let parent = name.rsplitn(2, '/').nth(1);
            if let Some(p) = parent {
                theme_name = Some(p.to_string());
            } else {
                // theme.json 在根目录，从 zip 文件名推断
                theme_name = Some(
                    zip_file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("imported")
                        .to_string(),
                );
            }
            break;
        }
    }

    let theme_name = theme_name.ok_or_else(|| {
        "Invalid theme package: theme.json not found in zip".to_string()
    })?;

    // 如果根目录的 theme.json 没有 parent，主题名用 zip 文件名
    // （上面 fallback 已处理）

    let target_dir = theme_state.themes_dir.join(&theme_name);
    if target_dir.exists() {
        // 覆盖：先删除旧目录
        fs::remove_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    // 解压所有文件
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = entry.name().replace('\\', "/");

        // 跳过 macOS 元数据目录
        if entry_name.contains("__MACOSX") || entry_name.ends_with(".DS_Store") {
            continue;
        }

        // 去除 zip 内的顶层目录前缀（如果它等于主题名）
        let rel_path = if entry_name.starts_with(&format!("{}/", theme_name)) {
            entry_name[theme_name.len() + 1..].to_string()
        } else if entry_name == "theme.json" {
            entry_name
        } else if entry_name.contains('/') {
            // 取 theme.json 之后的路径作为相对路径
            entry_name.to_string()
        } else {
            entry_name
        };

        if rel_path.is_empty() {
            continue;
        }

        let out_path = target_dir.join(&rel_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }

    Ok(theme_name)
}

// ============================================================
// 辅助函数（与 generator::load_site_config 类似，避免循环依赖）
// ============================================================

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
            toml::from_str::<SiteConfig>(&toml_str).map_err(|e| format!("parse site_config: {e}"))
        }
        None => Ok(SiteConfig::default()),
    }
}

fn save_site_config(db: &DbState, cfg: &SiteConfig) -> Result<(), String> {
    let toml_str = toml::to_string(cfg).map_err(|e| e.to_string())?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO site_config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params!["main", toml_str],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// tauri Emitter trait
use tauri::Emitter;
