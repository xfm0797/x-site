use crate::db;
use crate::db::DbState;
use crate::deploy;
use crate::error::AppError;
use crate::models::post::DeployTarget;
use std::collections::HashMap;
use tauri::{Emitter, State};

/// 保存部署目标列表
///
/// 只更新 SiteConfig.deploy_targets，避免与通用设置面板的整份配置互相覆盖。
#[tauri::command]
pub fn save_deploy_targets(
    app: tauri::AppHandle,
    db: State<'_, DbState>,
    targets: Vec<DeployTarget>,
) -> Result<(), String> {
    let mut cfg = db::config::load(&db).map_err(|e| e.to_frontend())?;
    cfg.deploy_targets = targets;
    db::config::save(&db, &cfg).map_err(|e| e.to_frontend())?;
    let _ = app.emit("deploy-targets://changed", ());
    Ok(())
}

/// 部署站点，config 支持 type=local|git|netlify|vercel 及对应配置项
///
/// - 通过 `emit("deploy://progress", payload)` 推送步骤（复用 generator 的 ProgressSink）
/// - payload: `{ current, total, percent, file }`（step 名放在 file 字段）
#[tauri::command]
pub async fn deploy(
    app: tauri::AppHandle,
    site_dir: String,
    config: HashMap<String, String>,
) -> Result<String, String> {
    // adapter: 把 AppHandle 转成 ProgressSink，和 generate_site 同模式
    struct TauriProgress<'a>(&'a tauri::AppHandle);
    impl crate::generator::ProgressSink for TauriProgress<'_> {
        fn emit(&self, current: usize, total: usize, file: &str) {
            let percent = if total == 0 {
                100
            } else {
                ((current as f64 / total as f64) * 100.0) as u32
            };
            let _ = self.0.emit(
                "deploy://progress",
                serde_json::json!({
                    "current": current,
                    "total": total,
                    "percent": percent,
                    "file": file,
                }),
            );
        }
    }
    let sink = TauriProgress(&app);

    deploy::deploy(&site_dir, &config, Some(&sink))
        .await
        .map_err(|e: AppError| e.to_frontend())
}
