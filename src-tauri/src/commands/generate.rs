use crate::db::DbState;
use crate::error::AppResult;
use crate::generator::{self, ProgressSink};
use tauri::{AppHandle, Emitter, Manager, State};

/// 读取静态站点输出目录（预览 / 同步页共享）
#[tauri::command]
pub fn get_output_dir(db: State<'_, DbState>) -> Result<String, String> {
    let cfg = crate::db::config::load(&db).map_err(|e| e.to_frontend())?;
    Ok(cfg.output_dir)
}

/// 保存静态站点输出目录（只回写该字段，避免覆盖其他配置）
#[tauri::command]
pub fn save_output_dir(db: State<'_, DbState>, dir: String) -> Result<(), String> {
    let mut cfg = crate::db::config::load(&db).map_err(|e| e.to_frontend())?;
    cfg.output_dir = dir;
    crate::db::config::save(&db, &cfg).map_err(|e| e.to_frontend())?;
    Ok(())
}

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
pub async fn generate_site(app: AppHandle, output_dir: String) -> Result<String, String> {
    // 从 AppHandle 解析 generator 所需路径（generator 模块本身不依赖 tauri）
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let paths = generator::GeneratorPaths {
        themes_dir: app_data_dir.join("themes"),
        user_assets_dir: app_data_dir.join("assets"),
    };

    // 生成是 CPU/IO 密集的同步流程，放到阻塞线程池，避免卡住 async runtime
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<DbState>();
        let sink = TauriProgress(&app);
        run_generator(&paths, &sink, &db, std::path::Path::new(&output_dir))
            .map_err(|e: crate::error::AppError| e.to_frontend())?;
        Ok::<String, String>(output_dir)
    })
    .await
    .map_err(|e| format!("Generate task failed: {e}"))?
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
