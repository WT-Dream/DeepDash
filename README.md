# DeepDash

DeepDash 1.0.0 是 `@deepseek-ai/dsh` 的 Windows 桌面启动器和全局版本管理器。它只负责管理 DSH Web 进程与 DSH 版本，不封装或修改 DSH 官方数据。

## 边界

- 只通过本机 npm 默认 global prefix 安装、升级和切换 `@deepseek-ai/dsh`。
- 只启动本启动器创建的 `dsh web` 子进程，并把 DSH Web 服务加载在自己的 Tauri 窗口中。
- 不自定义 npm prefix，不复制、迁移或删除 DSH 安装目录。
- 不读取或修改 DSH 配置、API Key、工作区、会话和插件目录。
- 只监听 `127.0.0.1`，启动时始终传入 `--no-open`。
- 仅发布 Windows MSI 安装包。
- 配置和 WebView2 数据统一保存在 `%LOCALAPPDATA%\\ai.deepseek.deepdash`，安装目录不保存运行数据。

## 使用方式

- 启动 DSH 后，窗口仅显示官方 DSH Web 页面。
- 在 DSH 运行时点击窗口关闭按钮，会停止服务并返回启动器主界面。
- 在主界面点击关闭按钮，应用会最小化到系统托盘；通过托盘菜单的“退出”关闭应用。
- 托盘菜单可显示启动器或重启 DSH 服务。
- 侧边栏保留“设置”入口；设置页使用“偏好设置”作为页面标题，界面文字统一使用微软雅黑。

## 安装与使用

从 GitHub Releases 下载 `DeepDash_1.0.0_x64_en-US.msi` 并安装。系统需要预先安装 Node.js、npm 和 `@deepseek-ai/dsh` 所需的运行环境；DeepDash 不捆绑 Node.js 或 npm。

启动后可在版本管理页安装或切换 DSH 版本。在 DSH Web 运行时关闭窗口会返回 DeepDash 主界面；再次关闭主界面会最小化到托盘，只有托盘菜单的“退出”才会结束应用。

设置页的“数据目录”按钮会打开 DeepDash 的用户数据目录。

## 开发

需要 Node.js、npm、Rust 和 Tauri 2 的系统依赖。项目优先使用系统环境中的 Node.js/npm，不捆绑运行时。

```powershell
npm install
npm run dev
npm run build
npm run tauri:dev
npm run tauri:build
```

Windows 会自动解析 `node.exe`、`npm.cmd`、`npm.bat` 和 `dsh.cmd`。开发构建不会把用户配置复制到项目目录。

## 发布

```powershell
npm ci
npm run build
npm run tauri:build
```

Tauri 配置只生成 Windows MSI。构建结果位于 `src-tauri/target/release/bundle/msi/`，发布前请确认没有将 `node_modules`、`dist`、`target` 或用户数据提交到 Git。

## 模块

Rust 模块分别负责配置、环境检测、npm registry 版本模型、全局安装、DSH 进程状态机、错误映射和 Tauri IPC。Vue 负责启动面板、版本管理页和设置页。
