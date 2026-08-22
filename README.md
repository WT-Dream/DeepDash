# DeepDash

## 1.0.7 更新内容

- 修复 DSH 页面右上角快捷图标收缩后透明区域仍覆盖 DSH 控件的问题。
- 快捷图标移至 `Session log` 按钮左侧，保持约 5px 间距。
- 收缩状态下仅快捷图标响应悬停，DSH 其他区域可正常点击。

DeepDash 是 Windows 桌面启动器，用于安装、切换和运行 [DeepSeek Harness（DSH）](https://github.com/deepseek-ai/deepseek-harness)。它提供原生窗口、版本管理和系统托盘控制，不封装或修改 DSH 官方 Web 界面。

## 功能

- 从 npm 官方 registry 查看并切换 DSH 版本
- 在 DeepDash 窗口中启动 DSH Web
- 支持返回主界面、最小化到托盘和托盘退出
- 支持重启 DSH 服务
- 提供端口、主题和数据目录设置
- 支持在可信私有 Wi-Fi 下从手机浏览器继续使用当前 DSH Web 会话
- 启动前检测 DSH 端口占用，并明确提示端口冲突
- 保留 DSH Web 自身的 HTML5 拖放处理
- 支持 DSH Web 的剪贴板、麦克风、通知、全屏和屏幕共享等交互能力（不授予摄像头）

## 系统要求

- Windows 10 或更高版本（x64）
- Node.js 和 npm
- Windows WebView2 Runtime

DeepDash 使用本机 npm 默认 global prefix，不捆绑 Node.js、npm 或 DSH 运行时。

## 安装

从 [Releases](https://github.com/WT-Dream/DeepDash/releases) 下载并运行：

`DeepDash_1.0.7_x64_en-US.msi`

安装后启动 DeepDash，在“版本管理”中安装或切换 DSH 版本，然后从启动面板启动 DSH Web。

关闭 DSH Web 窗口会返回 DeepDash 主界面；再次关闭主界面会最小化到托盘。通过托盘菜单的“退出”结束应用。

## 手机局域网访问

打开左侧“手机连接”，启用“手机局域网访问”，选择当前 Wi-Fi 对应的私有 IPv4 地址并保存。重启 DSH 后，在“手机连接”页面点击“显示二维码”即可按需生成二维码和访问地址。手机与电脑连接同一个可信 Wi-Fi 后，用浏览器扫码即可使用同一个 DSH Web 会话，完成扫码后可手动关闭二维码。

启用手机访问时，DSH 仍只监听 `127.0.0.1`，不会绑定 `0.0.0.0`。DeepDash 会在所选的本机私有 IPv4 地址上启动临时 TCP 转发，将手机连接转发到本机 DSH；停止 DSH 或退出 DeepDash 时转发也会关闭。桌面端继续使用回环地址，手机端使用二维码中的局域网地址。

DSH 沉浸页面右上角提供 DeepDash 快捷入口。鼠标移入小图标后，可直接打开手机二维码，关闭二维码不会退出 DSH；也可以返回 DeepDash 主界面，运行中的服务会保留，主界面按钮会变为“返回 DSH”。关闭窗口仍会按原逻辑停止 DSH 服务。

DSH 可操作当前项目，因此局域网访问不提供登录隔离。仅在家庭或受信任的专用网络中开启，不要使用公共 Wi-Fi、不要在路由器上做端口转发，也不要将该地址暴露到公网。若手机无法打开地址，请确认 Windows 网络配置为“专用网络”，并在防火墙提示中仅允许专用网络访问。

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
