//! 本地预览服务器
//!
//! 使用 axum + tower-http::ServeDir 托管 output/ 目录。
//! - start_preview: 启动异步服务，端口自动分配（1024-65535 中第一个可用）
//! - stop_preview: 中断服务
//! - 端口通过 Tauri State 共享，前端可通过 get_preview_port 查询

use crate::models::theme::PreviewState;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::runtime::Runtime;
use tower_http::services::ServeDir;

/// 启动本地预览服务器
///
/// @param output_dir 静态文件根目录（一般是 generate_site 的输出目录）
/// @returns 实际监听的端口
#[tauri::command]
pub fn start_preview(
    preview_state: State<'_, PreviewState>,
    output_dir: String,
) -> Result<u16, String> {
    // 如果已有服务在跑，先停掉
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

    // 找一个可用端口：从 40000 开始递增扫描，避免和系统服务冲突
    let port = find_available_port(40000).ok_or_else(|| "No available port".to_string())?;
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;

    // 用独立的 Tokio runtime 启动 axum（避免阻塞主线程）
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    let serve_dir = ServeDir::new(root.clone());

    let handle = rt.spawn(async move {
        let app = axum::Router::new()
            .fallback_service(serve_dir)
            // 允许目录列表（开发期方便）— ServeDir 默认会返回 index.html
            ;
        // 把 std::TcpListener 转为 tokio::TcpListener
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let _ = axum::serve(listener, app).await;
    });

    // 把 runtime 也保存到状态中以便 stop 时 shutdown
    // 简化实现：用 JoinHandle.abort() 中断（axum::serve 会响应 abort）
    {
        let mut server = preview_state.server.lock().map_err(|e| e.to_string())?;
        *server = Some(handle);
        let mut p = preview_state.port.lock().map_err(|e| e.to_string())?;
        *p = port;
    }

    // 释放 rt 守护线程：runtime 必须保持存活，否则 task 会被取消
    // 我们让 rt 在后台一直跑（不 drop），通过 abort handle 来停止
    std::mem::forget(rt);

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

/// 从指定端口开始扫描，返回第一个可用端口
fn find_available_port(start: u16) -> Option<u16> {
    (start..65535).find(|&p| {
        TcpListener::bind(("127.0.0.1", p)).is_ok()
    })
}

// 避免 Arc 未使用 warning
#[allow(unused_imports)]
use std::sync::Arc as _Arc;
