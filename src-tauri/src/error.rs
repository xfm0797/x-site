//! 统一错误类型
//!
//! 内部业务代码全部用 `AppError` 传播；在 Tauri `#[tauri::command]` 边界做一次
//! `map_err(|e| e.to_frontend())` 转回 `String`，让前端保持稳定契约的同时，后端内部
//! 可以用 `?` 清晰传播、也能按错误类型做差异化处理。
//!
//! 错误分类：
//! - `Db`：SQLite / 序列化 / 解析失败，带原始错误
//! - `Io`：文件系统读写失败
//! - `Tauri`：Tauri 框架路径 / 插件调用失败
//! - `Validation`：用户输入 / 资源标识校验失败（直接显示 message 给前端）
//! - `Deploy`：部署命令 / API 返回的业务失败
//! - `Template`：Tera / Markdown 渲染失败
//! - `Theme`：主题导入 / 加载失败

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // 预留 variant 便于 future use，Deploy/Theme 暂未在业务中构造
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("tauri runtime error: {0}")]
    Tauri(String),

    // Validation 类型直接显示 message，不附加前缀，便于把错误原因原样透传给前端
    #[error("{0}")]
    Validation(String),

    #[error("deploy error: {0}")]
    Deploy(String),

    #[error("template error: {0}")]
    Template(String),

    #[error("theme error: {0}")]
    Theme(String),

    /// 兼容已有 Result<_, String> 的渲染模块（render.rs），让 `?` 自动转换
    #[error("{0}")]
    Message(String),
}

// --- 便捷 From 实现 ---

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Db(format!("json: {e}"))
    }
}

impl From<toml::de::Error> for AppError {
    fn from(e: toml::de::Error) -> Self {
        Self::Db(format!("parse site_config: {e}"))
    }
}

impl From<toml::ser::Error> for AppError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Db(format!("serialize site_config: {e}"))
    }
}

impl From<tera::Error> for AppError {
    fn from(e: tera::Error) -> Self {
        Self::Template(e.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        // 已经是业务错误的 String，原样包成 Message 类型
        Self::Message(s)
    }
}

// 锁 poisoning：互斥锁被另一个线程 panic 后拿到毒锁，这里恢复 panic 原线程
impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::Db("database lock poisoned".into())
    }
}

// --- 对外兼容 ---

impl AppError {
    /// 把 AppError 转成前端可展示的字符串
    /// Tauri command 的返回契约仍是 Result<T, String>
    pub fn to_frontend(&self) -> String {
        // Validation / Message 已经是用户可读的 message
        match self {
            Self::Validation(msg) | Self::Message(msg) => msg.clone(),
            other => other.to_string(),
        }
    }
}

/// 便捷别名：内部业务代码的 Result
pub type AppResult<T> = Result<T, AppError>;
