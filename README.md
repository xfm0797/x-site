# x-site

> 跨平台静态博客桌面客户端 · 类 Gridea · v0.1.0
>
> 作者：**XFM** · <https://github.com/xfm0797>

基于 **Tauri v2 + Vue 3 + TypeScript + Rust** 构建。安装包 < 15MB、启动快、内存占用低。

支持 Windows / macOS / Linux 三平台，内置 Markdown 编辑器、静态站点生成、主题管理、本地预览、一键部署、自动更新。

---

## ✨ 功能特性

### 内容管理
- **文章 CRUD** — 创建/编辑/删除，草稿与已发布双状态
- **Markdown 编辑器** — CodeMirror 6 语法高亮 + markdown-it 实时预览 + highlight.js 代码高亮
- **自动保存** — 编辑停顿 1s 防抖写入 SQLite，状态栏显示「已保存」
- **拖拽图片** — 从系统拖入图片自动复制到资源目录并插入 Markdown 相对路径
- **元数据面板** — 标题、Slug、分类、标签、封面图、发布状态

### 站点生成
- **Tera 模板引擎** — 支持模板继承、变量插值、过滤器
- **pulldown-cmark** — Markdown → HTML，支持表格/脚注/任务列表/删除线
- **syntect 代码高亮** — 内嵌 base16-ocean.dark 主题，无需前端 CSS
- **rayon 并发渲染** — 多文章并行生成，进度通过 Tauri 事件实时推送
- **生成的产物**：
  - 首页（分页，`posts_per_page` 可配）
  - 文章页 `posts/<slug>.html`
  - 归档页 `archive.html`（按年份分组）
  - 标签页 `tags/<slug>.html`
  - 分类页 `categories/<slug>.html`
  - 友情链接页 `links.html`
  - `sitemap.xml` / `feed.xml` (RSS) / `robots.txt`

### 主题系统
- **默认主题** — 内置 `default` 主题，含 7 个 Tera 模板 + CSS
- **主题切换** — 一键激活，配置自动持久化
- **ZIP 导入** — 选择 zip 包，Rust 端解压到 `themes/` 目录
- **主题结构约定**：
  ```
  themes/{name}/
  ├── theme.json         # 元数据 (name/author/description/screenshot)
  ├── post.tera          # 文章页
  ├── index.tera         # 首页（分页）
  ├── archive.tera       # 归档页
  ├── tag.tera           # 标签页
  ├── category.tera      # 分类页
  ├── links.tera         # 友链页
  ├── base.tera          # 全站布局骨架（可被继承）
  └── assets/            # 静态资源（css/js/img）
  ```

### 部署
- **Local Copy** — 复制 `output/` 到本地目标路径
- **Git Push** — 推送到 GitHub Pages / 任意 Git 仓库（自定义分支）

### 本地预览
- Rust 端 `axum` 静态文件服务，端口自动扫描（40000+）
- iframe 内嵌预览 + 一键在系统浏览器打开
- 修改文章 → 重新生成 → 一键刷新

### 其他
- **菜单管理** — 内置项（首页/归档/友链）开关 + 自定义项 + 拖拽排序
- **标签聚合** — 标签云按文章数渐变字号，点击展开文章列表
- **多语言** — 简体中文 / 繁體中文 / English，随系统语言自动选择
- **自动更新** — tauri-plugin-updater + GitHub Releases 签名分发

---

## 🏗️ 技术栈

| 层 | 技术 |
|---|---|
| 前端 | Vue 3 `<script setup>` + TypeScript + Vite 8 + Pinia + Vue Router |
| 编辑器 | CodeMirror 6（语法高亮、低内存） |
| 预览 | markdown-it + highlight.js |
| 桌面框架 | Tauri v2 |
| 后端 | Rust 1.98 |
| 数据库 | SQLite（rusqlite bundled） |
| 模板引擎 | Tera |
| Markdown 解析 | pulldown-cmark |
| 代码高亮 | syntect（Rust 端预渲染） |
| 并发 | rayon（渲染）+ tokio（HTTP 预览服务） |
| HTTP 服务 | axum + tower-http ServeDir |
| Tauri 插件 | sql / dialog / fs / shell / notification / updater |

---

## 📁 项目结构

```
x-site/
├── src/                          # Vue 前端
│   ├── api/tauri.ts              # invoke 封装层（camelCase → snake_case）
│   ├── components/editor/
│   │   ├── CodeMirrorEditor.vue  # CodeMirror 6 Vue 封装
│   │   └── MarkdownPreview.vue   # markdown-it 实时预览（200ms 防抖）
│   ├── views/
│   │   ├── HomeView.vue          # 文章列表
│   │   ├── EditorView.vue        # 三栏编辑器（工具栏+编辑+预览+元数据）
│   │   ├── MenusView.vue         # 菜单管理
│   │   ├── TagsView.vue          # 标签云
│   │   ├── ThemeView.vue         # 主题管理
│   │   └── SettingsView.vue      # 生成/部署/预览
│   ├── stores/post.ts            # Pinia store（current + 防抖自动保存）
│   ├── i18n/                     # 多语言
│   │   ├── index.ts              # useI18n() + t() + setLocale()
│   │   └── locales/{zh-CN,zh-TW,en}.ts
│   ├── router/index.ts
│   ├── App.vue                   # 侧边栏 + 语言切换 + 作者署名
│   └── main.ts
│
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── lib.rs                # 插件注册 + DB 初始化 + 命令注册
│   │   ├── commands/
│   │   │   ├── posts.rs          # load_posts / save_post / import_image
│   │   │   ├── generate.rs       # generate_site
│   │   │   ├── deploy.rs         # deploy
│   │   │   ├── theme.rs          # list_themes / apply_theme / import_theme
│   │   │   └── preview.rs        # start_preview / stop_preview
│   │   ├── db/mod.rs             # SQLite 连接 + WAL + 建表迁移
│   │   ├── models/
│   │   │   ├── post.rs           # Post / PostMeta / FriendLink / SiteConfig
│   │   │   └── theme.rs         # ThemeMeta / Theme / ThemeState / PreviewState
│   │   ├── generator/
│   │   │   ├── mod.rs            # 主流程 + rayon 并发 + emit 进度
│   │   │   ├── context.rs        # Tera 上下文结构
│   │   │   ├── render.rs         # pulldown-cmark + syntect + Tera 渲染
│   │   │   ├── assets.rs         # 主题 assets 递归复制
│   │   │   └── feed.rs           # sitemap.xml / feed.xml / robots.txt
│   │   └── deploy/mod.rs         # local 复制 / git push
│   ├── themes/default/           # 内置主题
│   │   ├── theme.json
│   │   ├── base.tera / post.tera / index.tera / archive.tera
│   │   ├── tag.tera / category.tera / links.tera
│   │   └── assets/css/style.css
│   ├── capabilities/default.json # Tauri 权限清单
│   ├── signing/                  # 更新签名
│   │   ├── generate-key.ps1
│   │   └── latest.json.example
│   ├── .cargo/config.toml        # 链接器优化
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── .github/workflows/release.yml # 三平台 CI 构建 + Release
├── vite.config.ts                # esbuild + manualChunks + gzip/brotli
└── package.json
```

---

## 🚀 快速开始

### 环境要求

- **Node.js** ≥ 20
- **Rust** ≥ 1.77（推荐 stable 最新版）
- **系统依赖**：
  - **Windows**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（含 "Desktop development with C++" 工作负载）
  - **macOS**: Xcode Command Line Tools (`xcode-select --install`)
  - **Linux**: `libwebkit2gtk-4.1-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf`

### 开发

```bash
# 安装依赖
npm install

# 启动开发模式（Tauri + Vite HMR）
npm run tauri dev
```

### 构建

```bash
# 生产构建（前端 + Rust release）
npm run tauri build

# 仅构建前端（验证 TS + 产物体积）
npm run build
```

产物位置：`src-tauri/target/release/bundle/`

| 平台 | 产物 |
|---|---|
| Windows | `msi/x-site_*_x64_en-US.msi` + `nsis/x-site_*_x64-setup.exe` |
| macOS | `dmg/x-site_*_aarch64.dmg` |
| Linux | `deb/x-site_*_amd64.deb` + `appimage/x-site_*_amd64.AppImage` |

---

## 📦 体积优化

目标：安装包 < 15MB。当前实测 `x-site.exe` 裸二进制 = **12.92 MB**。

| 检查项 | 目标 | 实测 |
|---|---|---|
| 裸二进制 | < 13 MB | 12.92 MB ✅ |
| 前端 dist | < 5 MB | ~3.8 MB ✅ |
| CodeMirror chunk (gzip) | < 600 KB | 568 KB ✅ |
| MSI 安装包 | < 15 MB | 待 Release 验证 |

**优化措施**：
- Rust release profile: `opt-level=3 + lto=true + codegen-units=1 + panic=abort + strip=true`
- 链接器优化: Windows `/OPT:REF /OPT:ICF /LTCG`、macOS `-dead_strip`
- Tauri features 精简: `tauri = { features = [] }`（未开 devtools）
- `syntect` 关闭默认全量语法定义: `default-features = false, features = ["default-fancy"]`
- 前端 `manualChunks` 分块 + `esbuild` minify + 移除 `console.log`
- `vite-plugin-compression` 生成 `.gz` + `.br`

---

## 🔄 自动更新

### 配置签名密钥

```powershell
cd src-tauri
.\signing\generate-key.ps1
```

按提示：
1. 把输出的 **公钥** 填入 `tauri.conf.json` → `plugins.updater.pubkey`
2. 把 **私钥** 保存到 `signing/private.key`（已加入 `.gitignore`，切勿提交）
3. 在 GitHub 仓库 Settings → Secrets 添加：
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

### 更新流程

应用启动时检查 `https://github.com/xfm0797/x-site/releases/latest/download/latest.json`，对比版本号，若发现新版本则提示用户更新。`latest.json` 由 GitHub Actions 在发布时自动生成并上传。

---

## 🤖 CI/CD

[`.github/workflows/release.yml`](.github/workflows/release.yml) 实现三平台矩阵构建：

```
push tag v* → build (Windows/macOS/Linux 并行)
              → tauri build --target <triple>（自动签名）
              → 收集 msi/exe/dmg/AppImage/deb + .sig
           → release
              → 生成 latest.json
              → 发布到 GitHub Releases
```

### 触发发布

```bash
git tag v0.1.0
git push origin v0.1.0
# CI 自动构建三平台安装包并发布 Release
```

---

## 🌐 多语言

支持三种语言，随系统语言自动选择，可在侧边栏底部手动切换：

| Locale | 语言 | 文件 |
|---|---|---|
| `zh-CN` | 简体中文（基准） | [src/i18n/locales/zh-CN.ts](src/i18n/locales/zh-CN.ts) |
| `zh-TW` | 繁體中文 | [src/i18n/locales/zh-TW.ts](src/i18n/locales/zh-TW.ts) |
| `en` | English | [src/i18n/locales/en.ts](src/i18n/locales/en.ts) |

新增语言：在 `src/i18n/locales/` 添加翻译文件，在 `src/i18n/index.ts` 的 `LOCALES` 数组注册即可。

---

## 📝 Tauri Commands

前端通过 `src/api/tauri.ts` 封装层调用 Rust command（自动 camelCase → snake_case）：

| Command | 说明 |
|---|---|
| `load_posts` | 查询所有文章元数据 |
| `save_post` | UPSERT 保存文章 |
| `import_image` | 复制图片到资源目录，返回相对路径 |
| `generate_site` | 生成静态站点（事件推送进度） |
| `deploy` | 部署到本地路径或 Git 仓库 |
| `list_themes` | 列出所有主题 |
| `apply_theme` | 切换当前主题 |
| `import_theme` | 导入 ZIP 主题包 |
| `start_preview` | 启动本地 HTTP 预览服务 |
| `stop_preview` | 停止预览服务 |

---

## 🛠️ 推荐开发环境

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## 📄 License

MIT © 2026 [XFM](https://github.com/xfm0797)
