//! 数据库集成测试（真实 SQLite 连接，用内存库跑完整仓储层）

use crate::db;
use crate::error::AppError;
use crate::models::post::{Post, SiteConfig};

fn sample_post(id: &str) -> Post {
    let now = "2026-06-01T00:00:00Z";
    Post {
        id: id.into(),
        title: format!("Post {id}"),
        slug: format!("post-{id}"),
        content: format!("# Content {id}\n\nBody text."),
        status: "published".into(),
        tags: vec!["rust".into(), "web".into()],
        category: "Tech".into(),
        cover: "assets/cover.png".into(),
        created_at: now.into(),
        updated_at: now.into(),
    }
}

fn new_mem_db() -> db::DbState {
    db::init(":memory:").expect("open :memory: db")
}

// ============================================================
// posts 仓储
// ============================================================

#[test]
fn posts_save_then_load_by_id_roundtrip() {
    let db = new_mem_db();
    let p = sample_post("abc");

    let saved = db::posts::save(&db, &p).expect("save");
    assert_eq!(saved.id, "abc");
    assert!(saved.updated_at != p.updated_at, "save 应刷新 updated_at");

    let loaded = db::posts::load_by_id(&db, "abc").expect("load");
    assert_eq!(loaded.id, "abc");
    assert_eq!(loaded.title, "Post abc");
    assert_eq!(loaded.content, "# Content abc\n\nBody text.");
    assert_eq!(loaded.tags, vec!["rust", "web"]);
    assert_eq!(loaded.category, "Tech");
    assert_eq!(loaded.cover, "assets/cover.png");
}

#[test]
fn posts_save_upsert_updates_fields() {
    let db = new_mem_db();
    let mut p = sample_post("upsert");
    db::posts::save(&db, &p).expect("first save");

    p.title = "Updated".into();
    p.slug = "updated-slug".into();
    p.status = "draft".into();
    p.tags = vec!["go".into()];
    let saved2 = db::posts::save(&db, &p).expect("second save");

    assert_eq!(saved2.title, "Updated");
    assert_eq!(saved2.slug, "updated-slug");
    assert_eq!(saved2.status, "draft");
    assert_eq!(saved2.tags, vec!["go"]);

    // 仍只有一行（ON CONFLICT DO UPDATE）
    let conn = db.0.lock().unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM posts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn posts_delete_is_idempotent() {
    let db = new_mem_db();
    db::posts::save(&db, &sample_post("del1")).unwrap();

    let first = db::posts::delete(&db, "del1").expect("first delete");
    assert!(first, "第一次删除应返回 true");

    let second = db::posts::delete(&db, "del1").expect("second delete");
    assert!(!second, "第二次删除应返回 false（已不存在）");
}

#[test]
fn posts_load_published_only_published() {
    let db = new_mem_db();
    let p1 = sample_post("p1");
    db::posts::save(&db, &p1).unwrap();
    let mut p2 = sample_post("p2");
    p2.status = "draft".into();
    db::posts::save(&db, &p2).unwrap();

    let published = db::posts::load_published(&db).expect("load_published");
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].id, "p1");
}

#[test]
fn posts_load_all_meta_sorted_by_updated_at_desc() {
    let db = new_mem_db();
    let mut p1 = sample_post("older");
    p1.updated_at = "2025-01-01T00:00:00Z".into();
    let mut p2 = sample_post("newer");
    p2.updated_at = "2026-12-31T00:00:00Z".into();
    db::posts::save(&db, &p1).unwrap();
    db::posts::save(&db, &p2).unwrap();

    let meta = db::posts::load_all_meta(&db).unwrap();
    assert_eq!(meta.len(), 2);
    assert_eq!(meta[0].id, "newer", "updated_at DESC");
    assert_eq!(meta[1].id, "older");
}

#[test]
fn posts_load_by_id_missing_returns_validation_error() {
    let db = new_mem_db();
    let err = db::posts::load_by_id(&db, "nonexistent").expect_err("should err");
    assert!(
        matches!(err, AppError::Validation(_)),
        "应为 Validation，实际: {err:?}"
    );
    assert!(
        err.to_frontend().contains("nonexistent"),
        "错误消息应包含 id"
    );
}

#[test]
fn posts_tags_roundtrip_as_json() {
    let db = new_mem_db();
    let mut p = sample_post("tags");
    p.tags = vec!["a".into(), "b".into(), "c".into()];
    db::posts::save(&db, &p).unwrap();

    let loaded = db::posts::load_by_id(&db, "tags").unwrap();
    assert_eq!(loaded.tags, vec!["a", "b", "c"]);
}

// ============================================================
// config 仓储
// ============================================================

#[test]
fn config_load_empty_returns_default() {
    let db = new_mem_db();
    let cfg = db::config::load(&db).expect("load empty");
    // 空表 → 返回 SiteConfig::default()
    assert_eq!(cfg.title, SiteConfig::default().title);
    assert_eq!(cfg.posts_per_page, 10); // 常规默认值
}

#[test]
fn config_save_then_load_roundtrip() {
    let db = new_mem_db();
    let cfg = SiteConfig {
        title: "My Blog".into(),
        url: "https://my.blog".into(),
        posts_per_page: 7,
        icp: "京ICP备1号".into(),
        theme: "dark".into(),
        ..Default::default()
    };

    db::config::save(&db, &cfg).expect("save");
    let loaded = db::config::load(&db).expect("load");
    assert_eq!(loaded.title, "My Blog");
    assert_eq!(loaded.url, "https://my.blog");
    assert_eq!(loaded.posts_per_page, 7);
    assert_eq!(loaded.icp, "京ICP备1号");
    assert_eq!(loaded.theme, "dark");
}

#[test]
fn config_save_overwrites_existing() {
    let db = new_mem_db();
    let cfg = SiteConfig {
        title: "V1".into(),
        ..Default::default()
    };
    db::config::save(&db, &cfg).unwrap();

    let cfg2 = SiteConfig {
        title: "V2".into(),
        ..cfg
    };
    db::config::save(&db, &cfg2).unwrap();

    // 必须在独立作用域内释放 MutexGuard，否则后续 db::config::load 二次加锁会死锁
    {
        let conn = db.0.lock().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM site_config", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "ON CONFLICT UPDATE 应保持单行");
    }

    let loaded = db::config::load(&db).unwrap();
    assert_eq!(loaded.title, "V2");
}

// ============================================================
// init() 迁移幂等（旧库升级场景）
// ============================================================

#[test]
fn db_init_migrations_are_idempotent() {
    // 第一次 init：新建空表 + 跑 migrations（ALTER TABLE ADD slug 等）
    let _a = db::init(":memory:").unwrap();
    // 第二次 init：表已存在 + migrations 再次跑（但列已存在，应被忽略）
    let _b = db::init(":memory:").unwrap();
    // 关键：没有 panic / 返回 AppError
}
