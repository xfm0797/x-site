//! 站点配置仓储（site_config 表，单行 KV）
//!
//! 配置序列化为 TOML 存储，表中固定 `key="main"`。

use crate::error::AppResult;
use crate::models::post::SiteConfig;
use super::DbState;

/// 从 site_config 表读取主配置；缺失时返回默认值
pub fn load(db: &DbState) -> AppResult<SiteConfig> {
    let conn = db.0.lock()?;
    let mut stmt = conn.prepare("SELECT value FROM site_config WHERE key = ?1")?;
    let row: Option<String> = stmt
        .query_row(rusqlite::params!["main"], |r| r.get(0))
        .ok();
    match row {
        Some(toml_str) => Ok(toml::from_str::<SiteConfig>(&toml_str)?),
        None => Ok(SiteConfig::default()),
    }
}

/// 覆盖写回 site_config（INSERT ON CONFLICT UPDATE）
pub fn save(db: &DbState, cfg: &SiteConfig) -> AppResult<()> {
    let toml_str = toml::to_string(cfg)?;
    let conn = db.0.lock()?;
    conn.execute(
        "INSERT INTO site_config (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        rusqlite::params!["main", toml_str],
    )?;
    Ok(())
}
