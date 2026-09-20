#!/usr/bin/env node
/**
 * 自动递增 patch 版本号
 *
 * 在 pre-commit 钩子中调用：每次提交自动把版本号 patch +1，
 * 并同步写入 package.json / tauri.conf.json / Cargo.toml 三处，
 * 修改后自动 git add，使版本变更包含在本次提交中。
 */
import { readFileSync, writeFileSync, chmodSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { execSync } from 'node:child_process';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// 1. 读取当前版本（以 package.json 为唯一真相源）
const pkgPath = join(root, 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf-8'));
const current = pkg.version;

// 2. 解析 semver 并 bump patch
const parts = current.split('.');
if (parts.length !== 3 || parts.some((n) => isNaN(Number(n)))) {
  console.error(`skip bump: invalid version "${current}"`);
  process.exit(0);
}
const [major, minor, patch] = parts.map(Number);
const next = `${major}.${minor}.${patch + 1}`;

console.log(`bump version: ${current} -> ${next}`);

// 3. 写入 package.json（保留原缩进格式）
pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');

// 4. 写入 tauri.conf.json（仅替换第一个 "version" 字段）
const tauriPath = join(root, 'src-tauri', 'tauri.conf.json');
const tauriRaw = readFileSync(tauriPath, 'utf-8');
const tauriNew = tauriRaw.replace(
  /"version":\s*"[^"]*"/,
  `"version": "${next}"`
);
writeFileSync(tauriPath, tauriNew);

// 5. 写入 Cargo.toml（仅替换行首的包版本，不动依赖版本）
const cargoPath = join(root, 'src-tauri', 'Cargo.toml');
const cargoRaw = readFileSync(cargoPath, 'utf-8');
const cargoNew = cargoRaw.replace(
  /^version\s*=\s*"[^"]*"/m,
  `version = "${next}"`
);
writeFileSync(cargoPath, cargoNew);

// 6. git add 这三个文件，使变更纳入本次提交
try {
  execSync(
    'git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml',
    { cwd: root, stdio: 'pipe' }
  );
} catch {
  // 非 git 环境或无暂存区时静默跳过
}
