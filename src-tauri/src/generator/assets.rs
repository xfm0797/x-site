//! 主题资源目录复制（递归）

use std::fs;
use std::path::Path;

/// 递归复制整个目录
///
/// - 目标目录如果存在会被清空再写（避免旧文件残留）
/// - 隐藏文件（以 `.` 开头）跳过
pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let target = dst.join(&name);
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}
