//! 远程平台部署模块（Netlify / Vercel REST API）
//!
//! 将生成的静态站点目录通过平台 REST API 上传并发布：
//! - Netlify：创建 deploy → 逐文件 gzip 上传 → 自动发布
//! - Vercel：逐文件上传（内容哈希去重）→ 创建 production deployment

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzEncoder;
use flate2::Compression;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

/// 收集目录下所有文件的相对路径与绝对路径
fn collect_files(site_dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut files = Vec::new();
    walk_dir(site_dir, site_dir, &mut files)?;
    Ok(files)
}

fn walk_dir(base: &Path, current: &Path, files: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(base, &path, files)?;
        } else {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((rel, path));
        }
    }
    Ok(())
}

// ============================================================
// Netlify
// ============================================================

#[derive(Debug, Deserialize)]
struct NetlifyDeploy {
    id: String,
    #[serde(default)]
    url: Option<String>,
}

/// 部署到 Netlify：创建 deploy → 逐文件 gzip 上传
pub async fn deploy_netlify(
    site_dir: &str,
    config: &HashMap<String, String>,
) -> Result<String, String> {
    let token = config.get("token").ok_or("Missing 'token' in netlify config")?;
    let site_id = config.get("site_id").ok_or("Missing 'site_id' in netlify config")?;

    let client = reqwest::Client::new();
    let base = format!("https://api.netlify.com/api/v1/sites/{}/deploys", site_id);

    // 1. 创建 deploy（空 body，Netlify 返回 deploy_id）
    let resp = client
        .post(&base)
        .bearer_auth(token)
        .json(&serde_json::json!({ "title": "x-site deploy" }))
        .send()
        .await
        .map_err(|e| format!("netlify create deploy: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("netlify create deploy failed ({status}): {body}"));
    }

    let deploy: NetlifyDeploy = resp
        .json()
        .await
        .map_err(|e| format!("netlify parse deploy: {e}"))?;
    let deploy_id = &deploy.id;

    // 2. 逐文件 gzip 上传
    let site_path = Path::new(site_dir);
    let files = collect_files(site_path)?;
    let total = files.len();

    for (i, (rel_path, abs_path)) in files.iter().enumerate() {
        let raw = fs::read(abs_path).map_err(|e| format!("read {}: {e}", rel_path))?;
        // gzip 压缩
        let mut encoder = GzEncoder::new(&raw[..], Compression::default());
        let mut gz = Vec::new();
        encoder.read_to_end(&mut gz).map_err(|e| format!("gzip {}: {e}", rel_path))?;

        let upload_url = format!(
            "https://api.netlify.com/api/v1/deploys/{}/files/{}",
            deploy_id,
            url_encode_path(rel_path)
        );

        let resp = client
            .post(&upload_url)
            .bearer_auth(token)
            .header("Content-Encoding", "gzip")
            .body(gz)
            .send()
            .await
            .map_err(|e| format!("netlify upload {rel_path} ({i}/{total}): {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("netlify upload {rel_path} failed ({status}): {body}"));
        }
    }

    // 3. 完成
    Ok(format!("Netlify: uploaded {total} files (deploy {})", deploy_id))
}

// ============================================================
// Vercel
// ============================================================

#[derive(Debug, Deserialize)]
struct VercelFile {
    digest: String,
    #[allow(dead_code)]
    size: u64,
}

#[derive(Debug, Serialize)]
struct VercelDeployFile {
    file: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct VercelDeployment {
    url: String,
}

/// 部署到 Vercel：逐文件上传 → 创建 production deployment
pub async fn deploy_vercel(
    site_dir: &str,
    config: &HashMap<String, String>,
) -> Result<String, String> {
    let token = config.get("token").ok_or("Missing 'token' in vercel config")?;
    let project_id = config
        .get("project_id")
        .ok_or("Missing 'project_id' in vercel config")?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| format!("header: {e}"))?,
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("reqwest client: {e}"))?;

    let site_path = Path::new(site_dir);
    let files = collect_files(site_path)?;
    let total = files.len();
    let mut deploy_files: Vec<VercelDeployFile> = Vec::with_capacity(total);

    // 1. 逐文件上传到 /v2/files（Vercel 按内容哈希去重）
    for (i, (rel_path, abs_path)) in files.iter().enumerate() {
        let raw = fs::read(abs_path).map_err(|e| format!("read {}: {e}", rel_path))?;

        let resp = client
            .post("https://api.vercel.com/v2/files")
            .body(raw)
            .send()
            .await
            .map_err(|e| format!("vercel upload {rel_path} ({i}/{total}): {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("vercel upload {rel_path} failed ({status}): {body}"));
        }

        let f: VercelFile = resp
            .json()
            .await
            .map_err(|e| format!("vercel parse {rel_path}: {e}"))?;
        deploy_files.push(VercelDeployFile {
            file: rel_path.clone(),
            sha: f.digest,
        });
    }

    // 2. 创建 production deployment
    let mut body = serde_json::json!({
        "files": deploy_files,
        "project": project_id,
        "target": "production",
        "name": "x-site",
    });
    if let Some(team_id) = config.get("team_id") {
        body["teamId"] = serde_json::Value::String(team_id.clone());
    }

    let resp = client
        .post("https://api.vercel.com/v13/deployments")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("vercel create deployment: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("vercel create deployment failed ({status}): {b}"));
    }

    let dep: VercelDeployment = resp
        .json()
        .await
        .map_err(|e| format!("vercel parse deployment: {e}"))?;

    Ok(format!("Vercel: deployed {total} files → https://{}", dep.url))
}

// ============================================================
// 工具
// ============================================================

/// URL 编码路径（Netlify 文件上传路径需要），保留 /
fn url_encode_path(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            let mut s = String::new();
            for byte in seg.bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        s.push(byte as char);
                    }
                    _ => s.push_str(&format!("%{:02X}", byte)),
                }
            }
            s
        })
        .collect::<Vec<_>>()
        .join("/")
}
