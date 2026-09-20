#!/usr/bin/env node
/**
 * 安装 git pre-commit 钩子
 *
 * 通过 package.json 的 "prepare" 脚本在 npm install 时自动执行，
 * 无需额外依赖（husky 等）。钩子作用：每次提交自动递增 patch 版本号。
 */
import { writeFileSync, mkdirSync, existsSync, chmodSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const gitDir = join(root, '.git');

if (!existsSync(gitDir)) {
  console.log('skip hook install: not a git repository');
  process.exit(0);
}

const hooksDir = join(gitDir, 'hooks');
mkdirSync(hooksDir, { recursive: true });

const hookPath = join(hooksDir, 'pre-commit');
const hookContent = `#!/bin/sh
# x-site auto-bump: 每次提交自动递增 patch 版本号
# 由 scripts/install-hooks.mjs 安装（npm prepare 阶段）
node scripts/bump-version.mjs
`;

writeFileSync(hookPath, hookContent);
// Unix 设置可执行权限；Windows 无需（git for Windows 自带 sh 可执行）
try {
  chmodSync(hookPath, 0o755);
} catch {
  // Windows 无 chmod，忽略
}

console.log('git pre-commit hook installed: auto-bump patch version on every commit');
