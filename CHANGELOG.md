# Changelog

本文件记录 x-site 的版本变更，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Changed

- **版本号改为手动升级**：移除 `npm prepare` 安装的 pre-commit 自动 bump 钩子（删除 `scripts/install-hooks.mjs`）。`scripts/bump-version.mjs` 改为手动命令 `npm run version:bump -- <patch|minor|major|x.y.z>`，一次同步 `package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`（仅 `x-site` 包）；支持 `--dry-run`，并拒绝非法参数与降级。

## [0.1.13] - 2026-11-20

首个纳入版本记录与 CI 门禁的发布版本。

### Added

- **统一错误类型**：新增 `AppError` / `AppResult`，覆盖 Db / Io / Tauri / Validation / Deploy / Template / Theme / Message 八类错误，`to_frontend()` 统一打点并在返回前端时区分用户错误与内部错误。
- **仓储层**：新增 `db::config`（站点配置单行 KV，TOML 序列化）与 `db::posts`（文章 CRUD，tags 以 JSON 存储），命令层不再直接拼 SQL。
- **增量静态生成**：`.gen_state.json` 记录主题名与每篇文章的 slug + 内容哈希，未变更文章跳过 Markdown 与模板渲染；`clean_output` 自动清理已删除文章对应的“幽灵页面”。
- **生成器解耦**：新增 `GeneratorPaths` 与 `ProgressSink`，generator 不再依赖 Tauri，可直接单元测试；Tauri 命令层用适配器推送 `generate://progress`。
- **部署进度**：local / git / netlify / vercel 四种方式统一推送 `deploy://progress` 事件（current / total / percent / file）。
- **CI 门禁**：新增 GitHub Actions 工作流，Rust 侧 `cargo fmt --check` → `cargo clippy --all-targets -D warnings` → `cargo test --lib`；前端 `vue-tsc --noEmit` → `npm run build`。
- **测试**：新增 37 个后端单元 / 集成测试——仓储层 11 个（真实 SQLite 往返、幂等删除、排序、默认配置、迁移幂等）、generator 17 个（产物完整性、slug 冲突、二次生成幂等与幽灵页清理、状态文件）、主题导入路径校验 9 个。

### Changed

- **部署模块 AppResult 化**：`deploy` 与 `platform` 全部改为 `AppResult`，错误按 Validation / Deploy / Io 分类返回前端。
- **Git 部署幂等**：仅首次 `git init`；remote 采用 `get-url` 判断后 `add` 或 `set-url`；HEAD 未变化时跳过 `push`；移除 `--force`；改用 `tokio::process::Command` 避免阻塞运行时。
- **主题导入安全性**：zip 解压路径校验抽为纯函数 `safe_rel_path`，拒绝绝对路径、`..` 路径遍历与 Windows 盘符，并保留目标目录内 `starts_with` 双重校验。
- 全量 `cargo fmt` 规范化；CI 的 clippy 增加 `--all-targets`，确保测试代码同样受 lint 约束。

### Fixed

- 修复文章插图 / Logo 生成后 404：生成时同时复制主题 assets 与用户资源目录。
- 修复 `config_save_overwrites_existing` 测试中 `MutexGuard` 未释放导致的死锁。
- 修复 `config_load_empty_returns_default` 对默认标题的错误断言。
- 修复 Tera 不支持元组解构导致的归档模板渲染失败。
