use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 主题元数据（来自 theme.json）
///
/// 位于 themes/<name>/theme.json，用于在前端主题列表中显示信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMeta {
    /// 主题目录名（唯一标识）
    pub name: String,
    /// 显示名
    pub display_name: String,
    /// 作者
    pub author: String,
    /// 主题描述
    pub description: String,
    /// 版本号（如 "1.0.0"）
    pub version: String,
    /// 截图相对路径（如 "screenshot.png"，相对于主题目录）
    pub screenshot: String,
    /// 主页 URL（可选）
    pub homepage: Option<String>,
    /// 许可证（可选）
    pub license: Option<String>,
}

impl Default for ThemeMeta {
    fn default() -> Self {
        Self {
            name: String::new(),
            display_name: String::new(),
            author: String::new(),
            description: String::new(),
            version: "1.0.0".into(),
            screenshot: String::new(),
            homepage: None,
            license: None,
        }
    }
}

/// 完整主题（元数据 + 当前是否激活）
#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    #[serde(flatten)]
    pub meta: ThemeMeta,
    pub active: bool,
}

/// 主题管理状态：通过 Tauri State 管理
pub struct ThemeState {
    /// themes 目录绝对路径（app_data_dir/themes）
    pub themes_dir: PathBuf,
    /// 当前激活的主题名
    pub active: std::sync::Mutex<String>,
}

impl ThemeState {
    pub fn new(themes_dir: PathBuf, active: String) -> Self {
        Self {
            themes_dir,
            active: std::sync::Mutex::new(active),
        }
    }
}

/// 本地预览服务器状态：通过 Tauri State 管理
pub struct PreviewState {
    /// 当前运行的 axum 服务器句柄（关闭用）
    pub server: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// 当前监听端口
    pub port: std::sync::Mutex<u16>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            server: std::sync::Mutex::new(None),
            port: std::sync::Mutex::new(0),
        }
    }
}
