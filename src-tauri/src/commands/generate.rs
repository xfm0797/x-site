use crate::db::DbState;
use crate::error::AppResult;
use crate::generator::{self, ProgressSink};
use tauri::{AppHandle, Emitter, Manager, State};

/// 把 AppHandle 适配为 generator::ProgressSink（向前端 emit 进度事件）
struct TauriProgress<'a>(&'a AppHandle);
impl ProgressSink for TauriProgress<'_> {
    fn emit(&self, current: usize, total: usize, file: &str) {
        let percent = if total == 0 {
            100
        } else {
            ((current as f64 / total as f64) * 100.0) as u32
        };
        let _ = self.0.emit(
            "generate://progress",
            serde_json::json!({
                "current": current,
                "total": total,
                "percent": percent,
                "file": file,
            }),
        );
    }
}

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
    let output_path = std::path::PathBuf::from(&output_dir);

    // 从 AppHandle 解析 generator 所需路径（generator 模块本身不依赖 tauri）
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let paths = generator::GeneratorPaths {
        themes_dir: app_data_dir.join("themes"),
        user_assets_dir: app_data_dir.join("assets"),
    };
    let sink = TauriProgress(&app);

    run_generator(&paths, &sink, &db, &output_path)
        .map_err(|e: crate::error::AppError| e.to_frontend())?;
    Ok(output_dir)
}

/// generator 调用的薄封装，便于单独写测试（command 边界外可直接调用）
pub fn run_generator(
    paths: &generator::GeneratorPaths,
    progress: &dyn ProgressSink,
    db: &DbState,
    output_dir: &std::path::Path,
) -> AppResult<()> {
    generator::generate(paths, progress, db, output_dir.to_string_lossy().as_ref())
}
