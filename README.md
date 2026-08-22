# DeepDash

DeepDash 是 Windows 桌面启动器，用于安装、切换和运行 [DeepSeek Harness（DSH）](https://github.com/deepseek-ai/deepseek-harness)。它提供原生窗口、版本管理和系统托盘控制，不封装或修改 DSH 官方 Web 界面。

## 功能

- 从 npm 官方 registry 查看并切换 DSH 版本
- 在 DeepDash 窗口中启动 DSH Web
- 支持返回主界面、最小化到托盘和托盘退出
- 支持重启 DSH 服务
- 提供端口、主题和数据目录设置
- 启动前检测 DSH 端口占用，并明确提示端口冲突

## 系统要求

- Windows 10 或更高版本（x64）
- Node.js 和 npm
- Windows WebView2 Runtime

DeepDash 使用本机 npm 默认 global prefix，不捆绑 Node.js、npm 或 DSH 运行时。

## 安装

从 [Releases](https://github.com/WT-Dream/DeepDash/releases) 下载并运行：

`DeepDash_1.0.1_x64_en-US.msi`

安装后启动 DeepDash，在“版本管理”中安装或切换 DSH 版本，然后从启动面板启动 DSH Web。

关闭 DSH Web 窗口会返回 DeepDash 主界面；再次关闭主界面会最小化到托盘。通过托盘菜单的“退出”结束应用。

## 数据位置

DeepDash 的配置和 WebView2 应用数据保存在：

`%LOCALAPPDATA%\ai.deepseek.deepdash`

安装目录不保存运行数据。设置页的“数据目录”按钮可直接打开该目录。

## 开发

开发环境需要 Node.js、npm、Rust 和 Tauri 2 的 Windows 依赖。

```powershell
npm ci
npm run dev
npm run build
npm run tauri:dev
```

生成 Windows MSI：

```powershell
npm run tauri:build
```

构建结果位于 `src-tauri/target/release/bundle/msi/`。

## 许可证

[MIT License](LICENSE)
