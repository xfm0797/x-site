# UI 重构：主题合并设置 + 远程多平台部署 + 预览/同步

## Context（背景）

当前侧边栏有 7 个独立入口（文章/新建/菜单/标签/主题/通用设置/设置），设置页混合了生成站点、部署、本地预览三个功能。用户要求：
1. 把「通用设置」内容并入「主题」页（Tab 切换），并增加网站名称/描述设置
2. 把「设置」改为「远程」，功能为多平台部署配置（含 Vercel/Netlify API）
3. 把生成站点+启动预览移到语言切换上方，命名为「预览」
4. 新增「同步」在预览下方，功能：生成一次站点 + 依次部署到全部已配置远程目标

目标侧边栏结构：
```
文章 / 新建文章 / 菜单 / 标签 / 主题 / 远程   ← 主导航（separator 后）
预览 / 同步                                     ← 语言切换上方（separator 后）
[语言切换] / [XFM]                             ← 底部
```

## Phase 1：Rust 后端 — 多平台部署

### 1.1 Cargo.toml 新增依赖
- `reqwest = { version = "0.12", features = ["json", "stream"] }` — HTTP 客户端（Netlify/Vercel API）
- `flate2 = "1.0"` — gzip 压缩（Netlify 文件上传要求 gzip body）

### 1.2 models/post.rs — 扩展 SiteConfig
新增 `DeployTarget` 结构 + `deploy_targets` 字段：
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeployTarget {
    pub name: String,           // 目标名称，如 "GitHub Pages"
    pub config: HashMap<String, String>,  // type=local|git|netlify|vercel + 各类型字段
}
```
在 `SiteConfig` 加 `#[serde(default)] pub deploy_targets: Vec<DeployTarget>`，Default impl 加 `deploy_targets: Vec::new()`。

> 复用现有 `deploy()` 签名 `deploy(site_dir, &HashMap<String, String>)`——DeployTarget.config 直接传入，零改动。

### 1.3 deploy/mod.rs — 新增 netlify/vercel 分发
在 `deploy()` match 加两个分支，调用新模块 `deploy::platform`：
```rust
"netlify" => platform::deploy_netlify(site_dir, config).await,
"vercel" => platform::deploy_vercel(site_dir, config).await,
```

### 1.4 deploy/platform.rs — 新建平台部署模块
- `deploy_netlify(site_dir, config)`:
  - config 字段：`token`, `site_id`
  - 1) `POST https://api.netlify.com/api/v1/sites/{site_id}/deploys` → 拿 `deploy_id`
  - 2) 遍历 site_dir（复用 walk 模式），每个文件 gzip 后 `POST .../deploys/{deploy_id}/files/{rel_path}`，header `Authorization: Bearer {token}`, `Content-Encoding: gzip`
  - 3) Netlify 收齐文件后自动发布
- `deploy_vercel(site_dir, config)`:
  - config 字段：`token`, `project_id`（可选 `team_id`）
  - 1) 遍历文件，每个 `POST https://api.vercel.com/v2/files` body=原始字节 → 返回 `{sha, size}`
  - 2) `POST https://api.vercel.com/v13/deployments` body=`{files:[{file,sha}], project, target:"production"}` → 返回部署 URL

> 用 tokio::spawn + join 并发上传文件以加速（可选，先串行实现）。

## Phase 2：前端 — 类型与 API

### 2.1 src/api/tauri.ts
- 扩展 `DeployConfig` 支持 `type: "local"|"git"|"netlify"|"vercel"` + netlify/vercel 字段
- 新增 `DeployTarget` interface `{ name: string; config: Record<string,string> }`
- `SiteConfig` 加 `deploy_targets: DeployTarget[]`
- deploy_targets 持久化复用 `getThemeSettings()`/`saveThemeSettings()`（已在 SiteConfig 内）

## Phase 3：前端 — 视图重构

### 3.1 ThemeView.vue — Tab 化（主题 / 设置）
- 顶部加 `<div class="tabs">` 两个按钮切换 `activeTab` ref（"themes" | "settings"）
- Tab "主题"：保留现有主题网格 + 导入按钮
- Tab "设置"：合并 ThemeSettingsView 全部内容，并在最上方增加「网站名称」「网站描述」输入框（绑 `form.title` / `form.description`）
- onMounted 加载 getThemeSettings()，保存调用 saveThemeSettings(form)

### 3.2 删除 ThemeSettingsView.vue + 移除 /theme-settings 路由

### 3.3 RemoteView.vue — 新建（替换 SettingsView）
- 部署目标列表：每项显示 name + type badge + 编辑/删除按钮
- 新增目标表单：name + type 下拉（local/git/netlify/vercel）+ 动态字段
  - local: target 路径
  - git: repo, branch, message
  - netlify: token, site_id
  - vercel: token, project_id, (team_id)
- 保存到 SiteConfig.deploy_targets（通过 saveThemeSettings）

### 3.4 PreviewView.vue — 新建（生成 + 预览）
- 从 SettingsView 移出 generate section + preview section
- output_dir 输入 + 生成按钮 + 进度条 + iframe 预览 + 启动/停止/浏览器打开

### 3.5 SyncView.vue — 新建（同步：生成 + 部署全部远程目标）
- 显示已配置的 deploy_targets 列表（只读概览）
- 「开始同步」按钮：调用 generateSite → 成功后循环 deploy(outputDir, target.config) 依次部署每个目标
- 进度：先显示生成进度，再逐个显示部署结果（成功/失败 + URL）

### 3.6 删除 SettingsView.vue

## Phase 4：路由 + 侧边栏 + i18n

### 4.1 router/index.ts
- 移除 `/theme-settings`
- `/settings` → `/remote`（RemoteView）
- 新增 `/preview`（PreviewView）、`/sync`（SyncView）

### 4.2 App.vue 侧边栏
```
<RouterLink to="/">{{ t("nav.posts") }}</RouterLink>
<RouterLink to="/editor">{{ t("nav.newPost") }}</RouterLink>
<RouterLink to="/menus">{{ t("nav.menus") }}</RouterLink>
<RouterLink to="/tags">{{ t("nav.tags") }}</RouterLink>
<RouterLink to="/themes">{{ t("nav.themes") }}</RouterLink>
<RouterLink to="/remote" class="menu-section-end">{{ t("nav.remote") }}</RouterLink>
<RouterLink to="/preview" class="preview-section-end">{{ t("nav.preview") }}</RouterLink>
<RouterLink to="/sync">{{ t("nav.sync") }}</RouterLink>
<!-- sidebar-footer: 语言 + XFM -->
```

### 4.3 i18n（zh-CN / zh-TW / en 同步）
- nav: 删 `themeSettings`/`settings`，加 `remote`/`preview`/`sync`
- 新增 `remote.*`（title, addTarget, name, type, target, repo, branch, token, siteId, projectId, teamId, save, saved, empty, delete）
- `preview.*` 复用现有 `settings.generate.*` + `settings.preview.*` key
- 新增 `sync.*`（title, start, generating, deploying, success, failed, targetResult, done）
- 保留 `themeSettings.*`（ThemeView Tab 设置仍用这些 key）+ 新增 `themeSettings.siteName`/`siteDesc`

## 关键复用点
- `deploy()` 签名不变，DeployTarget.config 直接透传 → 零改动兼容现有 local/git
- `copy_dir_recursive()` 模式用于遍历上传文件
- `getThemeSettings()`/`saveThemeSettings()` 复用持久化 deploy_targets
- ThemeSettingsView 的表单逻辑/样式整体搬入 ThemeView Tab

## 验证
1. `cargo check` 通过
2. `npx vue-tsc --noEmit` 通过
3. `npm run tauri dev` 启动：
   - 主题页 Tab 切换正常，设置 Tab 能改网站名/描述/logo/评论并保存
   - 远程页能添加 local/git/netlify/vercel 四类目标并保存
   - 预览页能生成站点 + iframe 预览
   - 同步页一键生成+部署全部目标（先用 local 目标测试，netlify/vercel 需真实 token）
