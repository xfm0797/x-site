//! 主题管理命令：列出 / 切换 / 导入

use crate::db;
use crate::db::DbState;
use crate::error::AppError;
use crate::models::post::SiteConfig;
use crate::models::theme::{Theme, ThemeMeta, ThemeState};
use std::fs;
use std::io::Read;
use std::path::Path;
use tauri::{AppHandle, State};

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
        let mut meta: ThemeMeta = serde_json::from_str(&content).unwrap_or_default();
        // 目录名作为唯一 name
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        meta.name = dir_name;
        let is_active = meta.name == active;
        themes.push(Theme {
            meta,
            active: is_active,
        });
    }

    // 激活的排第一，然后按显示名
    themes.sort_by(|a, b| match (a.active, b.active) {
        (true, _) => std::cmp::Ordering::Less,
        (_, true) => std::cmp::Ordering::Greater,
        _ => a.meta.display_name.cmp(&b.meta.display_name),
    });

    // 从 db 拉取 site_config，覆盖 active 标记
    if let Ok(cfg) = db::config::load(&db) {
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
        return Err(AppError::Validation(format!("Theme '{}' not found", name)).to_frontend());
    }

    // 更新 SiteConfig
    let mut cfg = db::config::load(&db).map_err(|e| e.to_frontend())?;
    cfg.theme = name.clone();
    db::config::save(&db, &cfg).map_err(|e| e.to_frontend())?;

    // 同步 ThemeState
    {
        let mut active = theme_state
            .active
            .lock()
            .map_err(|e| AppError::Db(e.to_string()).to_frontend())?;
        *active = name.clone();
    }

    // 通知前端主题已切换
    let _ = app.emit("theme://changed", &name);
    Ok(())
}

/// 计算 zip 条目相对主题目录的路径
///
/// 返回 `Ok(None)` 表示应跳过该条目（macOS 元数据、空路径）。
/// 返回 `Err` 表示条目不安全：绝对路径、含 `..` 的路径遍历、Windows 盘符。
fn safe_rel_path(theme_name: &str, entry_name: &str) -> Result<Option<String>, String> {
    fn unsafe_path(p: &str) -> bool {
        let first = p.split('/').next().unwrap_or("");
        p.starts_with('/') || first.contains(':') || p.split('/').any(|c| c == "..")
    }

    let norm = entry_name.replace('\\', "/");

    // Zip Slip 防护：绝对路径 / 路径遍历 / 盘符一律拒绝
    if unsafe_path(&norm) {
        return Err(format!("Unsafe path in zip entry: {entry_name}"));
    }

    // 跳过 macOS 元数据目录
    if norm.contains("__MACOSX") || norm.ends_with(".DS_Store") {
        return Ok(None);
    }

    // 去除 zip 内的顶层目录前缀（如果它等于主题名）
    let rel_path = if norm.starts_with(&format!("{}/", theme_name)) {
        norm[theme_name.len() + 1..].to_string()
    } else {
        norm
    };

    if rel_path.is_empty() {
        return Ok(None);
    }
    // 去前缀后可能是绝对路径（如 `minimal//etc/passwd`）
    if unsafe_path(&rel_path) {
        return Err(format!("Unsafe path in zip entry: {entry_name}"));
    }
    Ok(Some(rel_path))
}

/// 从 zip 包导入主题（异步命令，解压在阻塞线程池执行）
///
/// 解压到 themes/.import-* 临时目录，限制条目数与解压量，成功后再原子替换
/// 目标主题目录，避免覆盖到一半失败导致旧主题被破坏。
#[tauri::command]
pub async fn import_theme(app: AppHandle, zip_path: String) -> Result<String, String> {
    let themes_dir = {
        use tauri::Manager;
        app.state::<ThemeState>().themes_dir.clone()
    };
    tauri::async_runtime::spawn_blocking(move || import_theme_blocking(&themes_dir, &zip_path))
        .await
        .map_err(|e| format!("Import task failed: {e}"))?
}

// 导入限额：防止 zip 炸弹（极小体积解压出海量文件）
const MAX_THEME_ENTRIES: usize = 5000;
const MAX_THEME_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_THEME_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

fn is_valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// import_theme 的同步实现（便于脱离 Tauri runtime 测试）
fn import_theme_blocking(themes_dir: &Path, zip_path: &str) -> Result<String, String> {
    let zip_file = Path::new(zip_path);
    if !zip_file.exists() {
        return Err(format!("File not found: {}", zip_path));
    }

    let file = fs::File::open(zip_file).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    if archive.len() > MAX_THEME_ENTRIES {
        return Err(format!(
            "Theme package has too many entries ({} > {})",
            archive.len(),
            MAX_THEME_ENTRIES
        ));
    }

    // 探测主题名：取第一个含 theme.json 的目录前缀
    let mut theme_name: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with("/theme.json") || name == "theme.json" {
            // 取 theme.json 的上一级目录名作为主题名
            match name.rsplit_once('/') {
                Some((p, _)) => theme_name = Some(p.to_string()),
                // theme.json 在根目录，从 zip 文件名推断
                None => {
                    theme_name = Some(
                        zip_file
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("imported")
                            .to_string(),
                    )
                }
            }
            break;
        }
    }

    let theme_name = theme_name
        .ok_or_else(|| "Invalid theme package: theme.json not found in zip".to_string())?;
    if !is_valid_theme_name(&theme_name) {
        return Err(format!("Invalid theme name: {theme_name}"));
    }

    fs::create_dir_all(themes_dir).map_err(|e| e.to_string())?;
    // 临时目录与目标目录同盘，后续 rename 才是原子操作
    let temp_dir = themes_dir.join(format!(
        ".import-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    if let Err(e) = extract_archive(&mut archive, &theme_name, &temp_dir) {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(e);
    }

    // 解压结果必须包含 theme.json，否则视为无效包
    if !temp_dir.join("theme.json").exists() {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err("Invalid theme package: theme.json not found after extraction".to_string());
    }

    replace_theme_dir(themes_dir, &theme_name, &temp_dir)?;
    Ok(theme_name)
}

/// 解压 zip 到临时目录，限制单文件与累计解压体积
fn extract_archive(
    archive: &mut zip::ZipArchive<fs::File>,
    theme_name: &str,
    dest: &Path,
) -> Result<(), String> {
    let mut total_bytes: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel_path) = safe_rel_path(theme_name, entry.name())? else {
            continue;
        };

        let out_path = dest.join(&rel_path);
        // 双保险：最终输出路径必须位于临时目录内
        if !out_path.starts_with(dest) {
            return Err(format!("Entry escapes theme directory: {rel_path}"));
        }

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // take(上限+1)：超过上限立即停止，避免超大条目撑爆磁盘
        let written = {
            let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            let mut limited = (&mut entry).take(MAX_THEME_FILE_BYTES + 1);
            std::io::copy(&mut limited, &mut out).map_err(|e| e.to_string())?
        };
        if written > MAX_THEME_FILE_BYTES {
            return Err(format!("File too large in theme package: {rel_path}"));
        }
        total_bytes = total_bytes.saturating_add(written);
        if total_bytes > MAX_THEME_TOTAL_BYTES {
            return Err(format!(
                "Theme package expands beyond {} MiB",
                MAX_THEME_TOTAL_BYTES / 1024 / 1024
            ));
        }
    }
    Ok(())
}

/// 用临时目录原子替换目标主题目录（失败时回滚旧主题）
fn replace_theme_dir(themes_dir: &Path, theme_name: &str, temp_dir: &Path) -> Result<(), String> {
    let target = themes_dir.join(theme_name);
    let backup = themes_dir.join(format!(
        ".backup-{}-{}",
        theme_name,
        chrono::Utc::now().timestamp_millis()
    ));
    let had_target = target.exists();
    if had_target {
        fs::rename(&target, &backup)
            .map_err(|e| format!("Failed to replace existing theme: {e}"))?;
    }
    match fs::rename(temp_dir, &target) {
        Ok(()) => {
            if had_target {
                let _ = fs::remove_dir_all(&backup);
            }
            Ok(())
        }
        Err(e) => {
            if had_target {
                let _ = fs::rename(&backup, &target);
            }
            let _ = fs::remove_dir_all(temp_dir);
            Err(format!("Failed to install theme: {e}"))
        }
    }
}

// ============================================================
// 通用主题设置：Logo / 页头 / 页尾 / 广告 / 评论系统
// ============================================================

/// 获取当前站点配置（前端用于通用设置面板）
#[tauri::command]
pub fn get_theme_settings(db: State<'_, DbState>) -> Result<SiteConfig, String> {
    db::config::load(&db).map_err(|e| e.to_frontend())
}

/// 保存站点配置（覆盖写回 site_config 表）
#[tauri::command]
pub fn save_theme_settings(
    app: AppHandle,
    db: State<'_, DbState>,
    config: SiteConfig,
) -> Result<(), String> {
    db::config::save(&db, &config).map_err(|e| e.to_frontend())?;
    let _ = app.emit("settings://changed", &config.theme);
    Ok(())
}

/// 上传 Logo 图片到 assets 目录
/// 复制源文件到 `<app_data_dir>/assets/logo-<ts>.<ext>`，
/// 返回相对路径 "assets/logo-xxx.png"，前端通过 convertFileSrc 展示
#[tauri::command]
pub fn upload_logo(app: AppHandle, src_path: String) -> Result<String, String> {
    use tauri::Manager;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let assets_dir = app_data_dir.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;

    let src = Path::new(&src_path);
    if !src.exists() {
        return Err(format!("File not found: {}", src_path));
    }

    // 推断扩展名
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let ts = chrono::Utc::now().timestamp_millis();
    let filename = format!("logo-{}.{}", ts, ext);
    let dest = assets_dir.join(&filename);

    fs::copy(src, &dest).map_err(|e| e.to_string())?;

    // 返回相对路径（用于 SiteConfig.logo 字段）
    Ok(format!("assets/{}", filename))
}

// tauri Emitter trait（仅 settings://changed 事件使用）
use tauri::Emitter;

#[cfg(test)]
mod tests {
    use super::{import_theme_blocking, safe_rel_path, MAX_THEME_FILE_BYTES};

    /// 构造一个测试用 zip 包
    fn make_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn imports_theme_replacing_stale_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes = tmp.path();
        // 旧版本主题遗留文件，导入后必须消失
        std::fs::create_dir_all(themes.join("demo")).unwrap();
        std::fs::write(themes.join("demo/stale.html"), "old").unwrap();

        let zip = themes.join("demo.zip");
        make_zip(
            &zip,
            &[
                ("demo/theme.json", b"{}"),
                ("demo/assets/style.css", b"body{}"),
            ],
        );

        let name = import_theme_blocking(themes, zip.to_str().unwrap()).unwrap();
        assert_eq!(name, "demo");
        assert!(themes.join("demo/theme.json").exists());
        assert!(themes.join("demo/assets/style.css").exists());
        assert!(!themes.join("demo/stale.html").exists());

        // 临时目录 / 备份目录不应残留
        let leftovers: Vec<String> = std::fs::read_dir(themes)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "不应残留临时目录: {leftovers:?}");
    }

    #[test]
    fn rejects_zip_slip_and_keeps_target_untouched() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes = tmp.path();
        let zip = themes.join("evil.zip");
        make_zip(
            &zip,
            &[
                ("demo/theme.json", b"{}"),
                ("demo/../../evil.html", b"boom"),
            ],
        );

        let err = import_theme_blocking(themes, zip.to_str().unwrap()).unwrap_err();
        assert!(err.contains("Unsafe path"), "{err}");
        assert!(!tmp.path().parent().unwrap().join("evil.html").exists());
        assert!(!themes.join("demo").exists(), "失败时不应创建目标主题目录");
    }

    #[test]
    fn rejects_oversized_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let themes = tmp.path();
        let big = vec![0u8; (MAX_THEME_FILE_BYTES + 1) as usize];
        let zip = themes.join("big.zip");
        make_zip(&zip, &[("demo/theme.json", b"{}"), ("demo/big.bin", &big)]);

        let err = import_theme_blocking(themes, zip.to_str().unwrap()).unwrap_err();
        assert!(err.contains("File too large"), "{err}");
        assert!(!themes.join("demo").exists());
    }

    #[test]
    fn strips_theme_name_prefix() {
        assert_eq!(
            safe_rel_path("minimal", "minimal/theme.json").unwrap(),
            Some("theme.json".into())
        );
        assert_eq!(
            safe_rel_path("minimal", "minimal/assets/style.css").unwrap(),
            Some("assets/style.css".into())
        );
    }

    #[test]
    fn keeps_root_theme_json() {
        assert_eq!(
            safe_rel_path("minimal", "theme.json").unwrap(),
            Some("theme.json".into())
        );
    }

    #[test]
    fn normalizes_backslashes() {
        assert_eq!(
            safe_rel_path("minimal", "minimal\\assets\\style.css").unwrap(),
            Some("assets/style.css".into())
        );
    }

    #[test]
    fn skips_macos_metadata() {
        assert_eq!(
            safe_rel_path("minimal", "minimal/__MACOSX/._theme.json").unwrap(),
            None
        );
        assert_eq!(safe_rel_path("minimal", "minimal/.DS_Store").unwrap(), None);
    }

    #[test]
    fn skips_empty_entry_for_theme_dir_itself() {
        assert_eq!(safe_rel_path("minimal", "minimal/").unwrap(), None);
    }

    #[test]
    fn rejects_parent_traversal() {
        let err = safe_rel_path("minimal", "../evil.html").unwrap_err();
        assert!(err.contains("Unsafe path"), "{err}");
        assert!(safe_rel_path("minimal", "minimal/../../evil.html").is_err());
        assert!(safe_rel_path("minimal", "assets/../../../evil.html").is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        assert!(safe_rel_path("minimal", "/etc/passwd").is_err());
        assert!(safe_rel_path("minimal", "minimal//etc/passwd").is_err());
    }

    #[test]
    fn rejects_windows_drive_path() {
        assert!(safe_rel_path("minimal", "C:/Windows/system32/evil.dll").is_err());
    }

    #[test]
    fn relative_entry_stays_inside_theme_dir() {
        // 所有通过校验的条目 join 到主题目录后都不会逃逸
        let target = std::path::Path::new("/themes/minimal");
        for entry in ["minimal/theme.json", "minimal/a/b.css", "theme.json"] {
            let rel = safe_rel_path("minimal", entry).unwrap().unwrap();
            assert!(target.join(rel).starts_with(target), "{entry}");
        }
    }
}
