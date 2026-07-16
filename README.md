# LanClip

[![CI](https://github.com/TimekeeperXY/LanClip/actions/workflows/ci.yml/badge.svg)](https://github.com/TimekeeperXY/LanClip/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-226541.svg)](LICENSE)

[产品网站](https://timekeeperxy.github.io/LanClip/) · [下载 Windows 版](https://github.com/TimekeeperXY/LanClip/releases/latest) · [反馈问题](https://github.com/TimekeeperXY/LanClip/issues)

LanClip 是一个 Windows 优先、为 macOS 预留兼容层的局域网剪贴板共享工具。两台可信设备完成一次性配对后，可在不经过云服务器的情况下自动同步文字和图片。

![LanClip 产品主视觉](website/assets/lanclip-network-hero.jpg)

## 当前 MVP

- Windows / macOS 通用的文字与 RGBA 图片剪贴板适配
- UDP 局域网设备自动发现
- 120 秒有效的 6 位一次性配对码
- 每对设备独立的 256 位共享密钥
- XChaCha20-Poly1305 认证加密传输
- 内容指纹与远端写入标记防止循环同步
- 配对设备地址持久化与入站连接地址学习，广播丢失时仍可双向发送
- 短暂连接失败自动重试一次
- 每 3 秒执行一次设备密钥认证心跳，在线状态反映真实可达性
- 图片传输记录显示内存缩略图，退出应用后自动清空
- Windows 兼容 PNG、CF_DIBV5、CF_DIB 与 CF_BITMAP 剪贴板图片格式
- 可选开机自启，登录后静默运行到系统托盘
- 可信设备密钥存储于 Windows Credential Manager / macOS Keychain
- 设置页提供防火墙诊断与一键修复
- NSIS 安装包自动维护局域网端口防火墙规则
- 20 MB 图片上限与 32 MB 网络帧上限
- 同步暂停、可信设备管理、设备重命名
- 仅驻留内存的最近传输摘要
- Windows 系统托盘入口

## 本地开发

需要 Node.js、Rust stable、Windows WebView2 与 Visual Studio C++ Build Tools。

```powershell
npm install
npm run tauri dev
```

仅预览前端界面：

```powershell
npm run dev
```

浏览器预览不会调用剪贴板和局域网能力。

## 构建安装包

仅生成可直接复制的 EXE：

```powershell
npm run tauri -- build --no-bundle
```

生成安装包：

```powershell
npm run tauri build
```

Tauri 会在 `src-tauri/target/release/bundle` 下生成 Windows 安装产物。

> 不要使用裸 `cargo build --release` 制作分发版本。它不会执行前端生产构建，也可能保留 `devUrl`，导致目标电脑尝试连接 `localhost:1420`。

## 两台设备测试

1. 两台电脑连接同一局域网，并允许 LanClip 通过 Windows 防火墙。
2. 在设备 A 点击“允许新设备”，保留显示的配对码。
3. 在设备 B 的“附近设备”中选择设备 A，输入配对码。
4. 两边显示为可信设备后，在任意一边复制文字或截图。
5. 到另一边粘贴，确认内容一致；再反向测试。

默认使用 UDP `44777` 端口发现设备，使用 TCP `44778` 端口进行配对和内容传输。

## 安全说明

剪贴板正文不写入本地历史文件，网络内容使用每对设备独立的密钥进行认证加密。MVP 的首次配对密钥由短时配对码派生，适合可信家庭或办公局域网；在公开或对抗性网络部署前，计划升级为 PAKE 配对协议，并加入设备指纹的双端确认。

应用配置存放在 Tauri 应用配置目录，但可信设备密钥不再写入 JSON。Windows 使用 Credential Manager，未来 macOS 构建使用 Keychain；卸载应用不会主动删除可信设备凭据和配对配置，以便升级安装保留设备关系，解除设备绑定时才删除对应凭据。

## 项目结构

```text
src/                      React 界面
src-tauri/src/clipboard.rs  跨平台剪贴板适配
src-tauri/src/crypto.rs     加密、密钥和内容指纹
src-tauri/src/network.rs    发现、配对和同步协议
src-tauri/src/state.rs      运行状态和配置持久化
src-tauri/src/model.rs      协议与界面数据模型
docs/DEVELOPMENT_PLAN.md    分阶段开发与验收计划
website/                    GitHub Pages 产品网站
```

## 开源协作

项目使用 [MIT License](LICENSE)。提交代码前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，安全问题请按 [SECURITY.md](SECURITY.md) 私下报告。
