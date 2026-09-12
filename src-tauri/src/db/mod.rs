use rusqlite::Connection;
use std::sync::Mutex;

/// 全局数据库状态，通过 Tauri State 管理
pub struct DbState(pub Mutex<Connection>);

/// 数据库建表 SQL（首次创建使用）
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS posts (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    slug        TEXT NOT NULL DEFAULT '',
    content     TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'draft',
    tags        TEXT NOT NULL DEFAULT '[]',
    category    TEXT NOT NULL DEFAULT '',
    cover       TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS site_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// 兼容性迁移 SQL：为旧表添加新列（如果列不存在）
/// 每条 `ALTER TABLE` 单独执行并忽略"列已存在"错误
const MIGRATIONS: &[&str] = &[
    "ALTER TABLE posts ADD COLUMN slug TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE posts ADD COLUMN category TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE posts ADD COLUMN cover TEXT NOT NULL DEFAULT ''",
];

/// 初始化数据库连接并执行迁移
pub fn init(db_path: &str) -> Result<DbState, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    // 启用 WAL 模式，提升并发读性能
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    // 逐条执行迁移（旧库升级用），新库会因列已存在而忽略错误
    for sql in MIGRATIONS {
        let _ = conn.execute(sql, []); // 忽略 "duplicate column" 错误
    }
    Ok(DbState(Mutex::new(conn)))
}
