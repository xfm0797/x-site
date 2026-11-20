use crate::deploy::{self, ProgressSink};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};

/// 把 AppHandle 适配为 deploy::ProgressSink（向前端 emit 部署进度事件）
struct TauriProgress<'a>(&'a AppHandle);
impl ProgressSink for TauriProgress<'_> {
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

/// 部署站点，config 支持 type=local|git|netlify|vercel 及对应配置项
///
/// - 通过 `emit("deploy://progress", payload)` 推送步骤
/// - payload: `{ current, total, percent, file }`（step 名放在 file 字段）
#[tauri::command]
pub async fn deploy(
    app: AppHandle,
    site_dir: String,
    config: HashMap<String, String>,
) -> Result<String, String> {
    let sink = TauriProgress(&app);
    deploy::deploy(&site_dir, &config, Some(&sink)).await
}
