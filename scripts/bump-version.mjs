#!/usr/bin/env node
/**
 * 手动升版本号
 *
 * 用法：
 *   npm run version:bump -- patch       # 0.1.13 -> 0.1.14
 *   npm run version:bump -- minor       # 0.1.13 -> 0.2.0
 *   npm run version:bump -- major       # 0.1.13 -> 1.0.0
 *   npm run version:bump -- 1.2.3       # 指定版本
 *   npm run version:bump -- patch --dry-run   # 只打印，不写文件
 *
 * 版本号唯一真相源是 package.json，一次同步写入 5 处：
 *   package.json / package-lock.json / src-tauri/tauri.conf.json /
 *   src-tauri/Cargo.toml / src-tauri/Cargo.lock（仅 x-site 包）
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const target = args.find((a) => !a.startsWith('--'));

function fail(msg) {
  console.error(`✖ ${msg}`);
  process.exit(1);
}

const pkgPath = join(root, 'package.json');
const pkgRaw = readFileSync(pkgPath, 'utf-8');
const pkg = JSON.parse(pkgRaw);
const current = pkg.version;

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;
if (!SEMVER.test(current)) {
  fail(`package.json 中的版本号不合法：${current}`);
}

if (!target) {
  fail('请指定 bump 类型（patch / minor / major）或目标版本号，例如：npm run version:bump -- patch');
}

let next;
if (['patch', 'minor', 'major'].includes(target)) {
  const [major, minor, patch] = current.split('.').map(Number);
  next =
    target === 'patch'
      ? `${major}.${minor}.${patch + 1}`
      : target === 'minor'
        ? `${major}.${minor + 1}.0`
        : `${major + 1}.0.0`;
} else if (SEMVER.test(target)) {
  next = target;
} else {
  fail(`无法识别的参数：${target}（应为 patch / minor / major 或 x.y.z）`);
}

if (next === current) {
  fail(`版本号已经是 ${current}，无需升版本`);
}

const [cmaj, cmin, cpat] = current.split('.').map(Number);
const [nmaj, nmin, npat] = next.split('.').map(Number);
if (nmaj < cmaj || (nmaj === cmaj && nmin < cmin) || (nmaj === cmaj && nmin === cmin && npat < cpat)) {
  fail(`不能降级版本号：${current} -> ${next}`);
}

const changes = [];
function writeFile(path, content) {
  changes.push(path.slice(root.length + 1));
  if (!dryRun) writeFileSync(path, content);
}

// 1. package.json —— 保留原缩进格式
pkg.version = next;
writeFile(pkgPath, JSON.stringify(pkg, null, 2) + '\n');

// 2. package-lock.json —— 根版本 + packages[""] 版本
const lockPath = join(root, 'package-lock.json');
const lock = JSON.parse(readFileSync(lockPath, 'utf-8'));
lock.version = next;
if (lock.packages?.['']) lock.packages[''].version = next;
writeFile(lockPath, JSON.stringify(lock, null, 2) + '\n');

// 3. tauri.conf.json —— 仅替换第一个 "version" 字段
const tauriPath = join(root, 'src-tauri', 'tauri.conf.json');
const tauriRaw = readFileSync(tauriPath, 'utf-8');
const tauriNew = tauriRaw.replace(/"version":\s*"[^"]*"/, `"version": "${next}"`);
if (tauriNew === tauriRaw) fail('tauri.conf.json 中未找到 version 字段');
writeFile(tauriPath, tauriNew);

// 4. Cargo.toml —— 仅替换行首的包版本，不动依赖版本
const cargoPath = join(root, 'src-tauri', 'Cargo.toml');
const cargoRaw = readFileSync(cargoPath, 'utf-8');
const cargoNew = cargoRaw.replace(/^version\s*=\s*"[^"]*"/m, `version = "${next}"`);
if (cargoNew === cargoRaw) fail('Cargo.toml 中未找到 package.version');
writeFile(cargoPath, cargoNew);

// 5. Cargo.lock —— 只改 x-site 自身，第三方 crate 的版本不动
const cargoLockPath = join(root, 'src-tauri', 'Cargo.lock');
const cargoLockRaw = readFileSync(cargoLockPath, 'utf-8');
const cargoLockNew = cargoLockRaw.replace(
  /(name = "x-site"\nversion = ")[^"]*"/,
  `$1${next}"`
);
if (cargoLockNew === cargoLockRaw) fail('Cargo.lock 中未找到 x-site 包');
writeFile(cargoLockPath, cargoLockNew);

console.log(`${dryRun ? '[dry-run] ' : ''}${current} -> ${next}`);
for (const f of changes) console.log(`  updated ${f}`);
if (dryRun) console.log('（未写入磁盘）');
