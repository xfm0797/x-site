//! 本地预览服务器
//!
//! 使用 axum + tower-http::ServeDir 托管 output/ 目录。
//! - start_preview: 启动异步服务，端口自动分配（从 40000 起第一个可用）
//! - stop_preview: 中断服务
//! - 服务运行在 Tauri 异步运行时上，句柄与端口通过 State 共享

use crate::models::theme::PreviewState;
use std::net::TcpListener;
use std::path::PathBuf;
use tauri::State;
use tower_http::services::ServeDir;

/// 启动本地预览服务器
///
/// @param output_dir 静态文件根目录（一般是 generate_site 的输出目录）
/// @returns 实际监听的端口
#[tauri::command]
pub async fn start_preview(
    preview_state: State<'_, PreviewState>,
    output_dir: String,
) -> Result<u16, String> {
    // 如果已有服务在跑，先停掉（abort 后任务与监听 socket 随 drop 释放）
    {
        let mut handle = preview_state.server.lock().map_err(|e| e.to_string())?;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    let root = PathBuf::from(&output_dir);
    if !root.exists() {
        return Err(format!("Output directory not found: {}", output_dir));
    }

    // 从 40000 起绑定第一个可用端口，listener 直接复用，避免扫描后重绑的竞态
    let listener = bind_available_port(40000).ok_or_else(|| "No available port".to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    // 在 async command（运行时上下文）中转换，失败可返回错误而非 panic
    let listener = tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;

    let serve_dir = ServeDir::new(root);

    // 在 Tauri 异步运行时上 spawn（不再自建并泄漏 Runtime）；
    // tokio::spawn 返回的 JoinHandle 与 PreviewState.server 类型一致
    let handle = tokio::spawn(async move {
        let app = axum::Router::new().fallback_service(serve_dir);
        let _ = axum::serve(listener, app).await;
    });

    {
        let mut server = preview_state.server.lock().map_err(|e| e.to_string())?;
        *server = Some(handle);
        let mut p = preview_state.port.lock().map_err(|e| e.to_string())?;
        *p = port;
    }

    Ok(port)
}

/// 停止本地预览服务器
#[tauri::command]
pub fn stop_preview(preview_state: State<'_, PreviewState>) -> Result<(), String> {
    let mut handle = preview_state.server.lock().map_err(|e| e.to_string())?;
    if let Some(h) = handle.take() {
        h.abort();
    }
    let mut p = preview_state.port.lock().map_err(|e| e.to_string())?;
    *p = 0;
    Ok(())
}

/// 获取当前预览端口（0 表示未启动）
#[tauri::command]
pub fn get_preview_port(preview_state: State<'_, PreviewState>) -> Result<u16, String> {
    let p = preview_state.port.lock().map_err(|e| e.to_string())?;
    Ok(*p)
}

/// 从指定端口开始扫描，返回第一个绑定成功的 listener
fn bind_available_port(start: u16) -> Option<TcpListener> {
    (start..65535).find_map(|p| TcpListener::bind(("127.0.0.1", p)).ok())
}
