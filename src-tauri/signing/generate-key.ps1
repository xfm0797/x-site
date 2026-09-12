# signing/generate-key.ps1
# 生成 Tauri 更新签名密钥对
# 用法：在 src-tauri 目录下运行 .\signing\generate-key.ps1
#
# 此脚本会：
# 1. 用 tauri signer 生成密钥对（公钥 + 私钥）
# 2. 把公钥写入 tauri.conf.json 的 plugins.updater.pubkey
# 3. 把私钥保存到 signing/private.key（务必加入 .gitignore，不要提交！）
# 4. 输出 GitHub Actions 需要的 TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD

param(
    [string]$KeyPassword = ""
)

$ErrorActionPreference = "Stop"

# 确保在 src-tauri 目录运行
Set-Location $PSScriptRoot
Set-Location ..

Write-Host "=== Tauri Signer Key Generation ===" -ForegroundColor Cyan

if ($KeyPassword -eq "") {
    $secure = Read-Host "Enter password for the signing key (leave empty for no password)" -AsSecureString
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
        [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    )
} else {
    $plain = $KeyPassword
}

# 运行 tauri signer generate
$args = @("signer", "generate")
if ($plain -ne "") {
    $args += "--password"
    $args += $plain
}

Write-Host "Running: tauri $($args -join ' ')"
& tauri @args

Write-Host ""
Write-Host "=== Next Steps ===" -ForegroundColor Green
Write-Host "1. Copy the PUBLIC KEY from above into tauri.conf.json -> plugins.updater.pubkey"
Write-Host "2. Save the PRIVATE KEY to signing/private.key  (already .gitignored)"
Write-Host "3. Set these GitHub Repository Secrets:"
Write-Host "   TAURI_SIGNING_PRIVATE_KEY = <private key content>"
if ($plain -ne "") {
    Write-Host "   TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $plain"
} else {
    Write-Host "   TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (empty)"
}
Write-Host "4. Re-run: npm run tauri build"
Write-Host "   The updater signature (.sig) will be emitted alongside each bundle."
