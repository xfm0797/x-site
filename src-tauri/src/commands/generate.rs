use crate::db::DbState;
use crate::generator;
use tauri::{AppHandle, State};

/// 生成静态站点
///
/// - 通过 `emit("generate://progress", payload)` 推送进度
/// - payload: `{ current, total, percent, file }`
///
/// 前端监听示例：
/// ```ts
/// import { listen } from "@tauri-apps/api/event";
/// const un = await listen<{current:number; total:number; percent:number; file:string}>(
///   "generate://progress",
///   e => console.log(e.payload.percent + "%", e.payload.file),
/// );
/// ```
#[tauri::command]
pub fn generate_site(
    app: AppHandle,
    db: State<'_, DbState>,
    output_dir: String,
) -> Result<String, String> {
    generator::generate(&app, &db, &output_dir)?;
    Ok(output_dir)
}
