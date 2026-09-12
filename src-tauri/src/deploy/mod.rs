use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// 部署静态站点，根据 config.type 选择部署方式
/// - local: 复制到本地目录
/// - git: 推送到 Git 远程仓库（GitHub Pages 等）
pub async fn deploy(
    site_dir: &str,
    config: &HashMap<String, String>,
) -> Result<String, String> {
    let deploy_type = config.get("type").map(|s| s.as_str()).unwrap_or("local");

    match deploy_type {
        "local" => deploy_local(site_dir, config),
        "git" => deploy_git(site_dir, config).await,
        other => Err(format!("Unknown deploy type: {}", other)),
    }
}

/// 本地部署：将 site_dir 复制到 config.target 路径
fn deploy_local(site_dir: &str, config: &HashMap<String, String>) -> Result<String, String> {
    let target = config.get("target").ok_or("Missing 'target' path in config")?;
    let source = Path::new(site_dir);
    let target_path = Path::new(target);

    if !source.exists() {
        return Err(format!("Source directory not found: {}", site_dir));
    }

    copy_dir_recursive(source, target_path)?;
    Ok(format!("Deployed to {}", target))
}

/// Git 部署：初始化 git 仓库并推送到远程
async fn deploy_git(site_dir: &str, config: &HashMap<String, String>) -> Result<String, String> {
    let repo = config.get("repo").ok_or("Missing 'repo' URL in config")?;
    let branch = config.get("branch").map(|s| s.as_str()).unwrap_or("gh-pages");
    let message = config.get("message").map(|s| s.as_str()).unwrap_or("Deploy site");

    // 逐条执行 git 命令
    let git_commands = [
        vec!["init"],
        vec!["remote", "add", "origin", repo],
        vec!["add", "."],
        vec!["commit", "-m", message],
        vec!["push", "-u", "origin", branch, "--force"],
    ];

    for args in &git_commands {
        let output = Command::new("git")
            .args(args)
            .current_dir(site_dir)
            .output()
            .map_err(|e| format!("Failed to run git {}: {}", args.join(" "), e))?;

        // commit 可能因为没有改动而失败，忽略该错误
        if !output.status.success() && args[0] != "commit" {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
        }
    }

    Ok(format!("Deployed to {} on branch {}", repo, branch))
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
