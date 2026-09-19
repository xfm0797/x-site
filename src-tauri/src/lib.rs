mod commands;
mod db;
mod deploy;
mod generator;
mod models;

use models::theme::{PreviewState, ThemeState};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        // updater 需在 tauri.conf.json 配置 pubkey 后启用
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // app data 目录用于存储数据库 + 主题 + 资源
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            // 初始化 SQLite
            let db_path = app_data_dir.join("xsite.db");
            let db_state = db::init(&db_path.to_string_lossy())?;

            // 从数据库加载 SiteConfig 获取当前主题名
            let site_config = load_config_from_db(&db_state).unwrap_or_default();
            let active_theme = site_config.theme.clone();

            // 主题目录：app_data_dir/themes/
            let themes_dir = app_data_dir.join("themes");
            std::fs::create_dir_all(&themes_dir)?;

            // 首次运行：把内置 default 主题释放到 themes/default/
            let default_theme_dir = themes_dir.join("default");
            if !default_theme_dir.exists() {
                extract_default_theme(&default_theme_dir)?;
            }

            // 首次运行：把内置 business 主题释放到 themes/business/
            let business_theme_dir = themes_dir.join("business");
            if !business_theme_dir.exists() {
                extract_business_theme(&business_theme_dir)?;
            }

            // 注册 Tauri State
            app.manage(db_state);
            app.manage(ThemeState::new(themes_dir, active_theme));
            app.manage(PreviewState::default());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::posts::load_posts,
            commands::posts::save_post,
            commands::posts::import_image,
            commands::generate::generate_site,
            commands::deploy::deploy,
            commands::theme::list_themes,
            commands::theme::apply_theme,
            commands::theme::import_theme,
            commands::theme::get_theme_settings,
            commands::theme::save_theme_settings,
            commands::theme::upload_logo,
            commands::preview::start_preview,
            commands::preview::stop_preview,
            commands::preview::get_preview_port,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 从数据库加载 SiteConfig（setup 时用，避免循环依赖）
fn load_config_from_db(
    db: &db::DbState,
) -> Result<models::post::SiteConfig, String> {
    use rusqlite::params;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM site_config WHERE key = ?1")
        .map_err(|e| e.to_string())?;
    let row: Option<String> = stmt
        .query_row(params!["main"], |r| r.get(0))
        .ok();
    match row {
        Some(toml_str) => toml::from_str(&toml_str)
            .map_err(|e| format!("parse site_config: {e}")),
        None => Ok(models::post::SiteConfig::default()),
    }
}

/// 把编译期内联的 default 主题释放到目标目录
fn extract_default_theme(target: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(target.join("assets").join("css"))
        .map_err(|e| e.to_string())?;

    // theme.json
    std::fs::write(
        target.join("theme.json"),
        include_str!("../themes/default/theme.json"),
    )
    .map_err(|e| e.to_string())?;

    // Tera 模板
    let templates = [
        ("base.tera", include_str!("../themes/default/base.tera")),
        ("post.tera", include_str!("../themes/default/post.tera")),
        ("index.tera", include_str!("../themes/default/index.tera")),
        ("archive.tera", include_str!("../themes/default/archive.tera")),
        ("tag.tera", include_str!("../themes/default/tag.tera")),
        ("category.tera", include_str!("../themes/default/category.tera")),
        ("links.tera", include_str!("../themes/default/links.tera")),
    ];
    for (name, content) in templates {
        std::fs::write(target.join(name), content).map_err(|e| e.to_string())?;
    }

    // CSS
    std::fs::write(
        target.join("assets").join("css").join("style.css"),
        include_str!("../themes/default/assets/css/style.css"),
    )
    .map_err(|e| e.to_string())?;

    // screenshot 占位（实际项目可换成真实截图）
    // 这里用空文件占位，前端会 fallback 显示主题名
    std::fs::write(target.join("screenshot.png"), []).ok();

    Ok(())
}

/// 把编译期内联的 business 主题释放到目标目录（产品展示型企业主题）
fn extract_business_theme(target: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(target.join("assets").join("css"))
        .map_err(|e| e.to_string())?;

    // theme.json
    std::fs::write(
        target.join("theme.json"),
        include_str!("../themes/business/theme.json"),
    )
    .map_err(|e| e.to_string())?;

    // Tera 模板（business 主题含产品列表页 product.tera）
    let templates = [
        ("base.tera", include_str!("../themes/business/base.tera")),
        ("post.tera", include_str!("../themes/business/post.tera")),
        ("index.tera", include_str!("../themes/business/index.tera")),
        ("archive.tera", include_str!("../themes/business/archive.tera")),
        ("tag.tera", include_str!("../themes/business/tag.tera")),
        ("category.tera", include_str!("../themes/business/category.tera")),
        ("links.tera", include_str!("../themes/business/links.tera")),
        ("product.tera", include_str!("../themes/business/product.tera")),
    ];
    for (name, content) in templates {
        std::fs::write(target.join(name), content).map_err(|e| e.to_string())?;
    }

    // CSS
    std::fs::write(
        target.join("assets").join("css").join("style.css"),
        include_str!("../themes/business/assets/css/style.css"),
    )
    .map_err(|e| e.to_string())?;

    std::fs::write(target.join("screenshot.png"), []).ok();

    Ok(())
}
