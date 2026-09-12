use crate::deploy;
use std::collections::HashMap;

/// 部署站点，config 支持 type=local|git 及对应配置项
#[tauri::command]
pub async fn deploy(
    site_dir: String,
    config: HashMap<String, String>,
) -> Result<String, String> {
    deploy::deploy(&site_dir, &config).await
}
