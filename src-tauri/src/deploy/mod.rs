use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub mod platform;

/// 复用 generator 的进度 trait，deploy 模块也能以 trait 对象接入
pub use crate::generator::ProgressSink;

/// 部署静态站点，根据 config.type 选择部署方式
/// - local: 复制到本地目录
/// - git: 推送到 Git 远程仓库（GitHub Pages 等）
/// - netlify: 通过 Netlify REST API 上传
/// - vercel: 通过 Vercel REST API 上传
///
/// `progress` 可选：传入 trait 对象即可通过 `deploy://progress` 事件推送当前步骤，
/// 不传时仍能正常部署（静默模式，适用于 CLI 或单元测试）。
pub async fn deploy(
    site_dir: &str,
    config: &HashMap<String, String>,
    progress: Option<&dyn ProgressSink>,
) -> Result<String, String> {
    let deploy_type = config.get("type").map(|s| s.as_str()).unwrap_or("local");

    if let Some(p) = progress {
        p.emit(0, 1, &format!("Starting {deploy_type} deploy..."));
    }

    match deploy_type {
        "local" => deploy_local(site_dir, config, progress),
        "git" => deploy_git(site_dir, config, progress).await,
        "netlify" => platform::deploy_netlify(site_dir, config, progress).await,
        "vercel" => platform::deploy_vercel(site_dir, config, progress).await,
        other => Err(format!("Unknown deploy type: {}", other)),
    }
}

/// 本地部署：将 site_dir 复制到 config.target 路径
fn deploy_local(
    site_dir: &str,
    config: &HashMap<String, String>,
    progress: Option<&dyn ProgressSink>,
) -> Result<String, String> {
    let target = config.get("target").ok_or("Missing 'target' path in config")?;
    let source = Path::new(site_dir);
    let target_path = Path::new(target);

    if !source.exists() {
        return Err(format!("Source directory not found: {}", site_dir));
    }

    if let Some(p) = progress {
        p.emit(0, 1, format!("Copying to {target}...").as_str());
    }
    copy_dir_recursive(source, target_path)?;
    if let Some(p) = progress {
        p.emit(1, 1, "Done");
    }
    Ok(format!("Deployed to {}", target))
}

/// Git 部署：初始化 git 仓库并推送到远程（可重复执行）
async fn deploy_git(
    site_dir: &str,
    config: &HashMap<String, String>,
    progress: Option<&dyn ProgressSink>,
) -> Result<String, String> {
    let repo = config.get("repo").ok_or("Missing 'repo' URL in config")?;
    let branch = config.get("branch").map(|s| s.as_str()).unwrap_or("gh-pages");
    let message = config.get("message").map(|s| s.as_str()).unwrap_or("Deploy site");

    if let Some(p) = progress {
        p.emit(0, 5, "Preparing git repository...");
    }

    // 仅首次部署初始化仓库
    if !Path::new(site_dir).join(".git").exists() {
        run_git(site_dir, &["init"]).await?;
    }

    // origin 已存在则更新 URL，否则添加，保证重复部署幂等
    if run_git(site_dir, &["remote", "get-url", "origin"]).await.is_ok() {
        run_git(site_dir, &["remote", "set-url", "origin", repo]).await?;
    } else {
        run_git(site_dir, &["remote", "add", "origin", repo]).await?;
    }

    // 仓库级提交身份（仅在未配置时写入，不污染全局 git config）
    if run_git(site_dir, &["config", "user.name"]).await.is_err() {
        run_git(site_dir, &["config", "user.name", "x-site"]).await?;
    }
    if run_git(site_dir, &["config", "user.email"]).await.is_err() {
        run_git(site_dir, &["config", "user.email", "x-site@localhost"]).await?;
    }

    if let Some(p) = progress {
        p.emit(1, 5, "Staging files...");
    }
    run_git(site_dir, &["add", "."]).await?;

    // commit：工作区无改动时跳过，其余错误如实上报
    match run_git(site_dir, &["commit", "-m", message]).await {
        Ok(_) => {
            if let Some(p) = progress {
                p.emit(2, 5, "Committed changes");
            }
        }
        Err(e) if e.contains("nothing to commit") => {
            if let Some(p) = progress {
                p.emit(2, 5, "No changes to commit");
            }
        }
        Err(e) => return Err(e),
    }

    if let Some(p) = progress {
        p.emit(3, 5, format!("Pushing to {repo} branch {branch}...").as_str());
    }
    // 不使用 --force，避免覆盖远程历史；非快进推送时返回 git 原始错误
    run_git(site_dir, &["push", "-u", "origin", branch]).await?;

    if let Some(p) = progress {
        p.emit(5, 5, "Done");
    }
    Ok(format!("Deployed to {} on branch {}", repo, branch))
}

/// 在站点目录执行 git 命令，成功返回 stdout，失败返回含 stdout/stderr 的错误
async fn run_git(site_dir: &str, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(site_dir)
        .output()
        .await
        .map_err(|e| format!("Failed to run git {}: {}", args.join(" "), e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(stdout.to_string())
    } else {
        // git commit 的 "nothing to commit" 打在 stdout，需一并纳入错误文本
        Err(format!(
            "git {} failed: {}{}",
            args.join(" "),
            stdout.trim(),
            stderr.trim()
        ))
    }
}

/// 递归复制目录
fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let dest = target.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
