# 参与 LanClip

感谢你愿意改进 LanClip。提交代码前，请先确认改动没有削弱局域网边界、配对认证和剪贴板隐私。

## 开发环境

- Node.js 20 或更高版本
- Rust stable
- Windows WebView2
- Visual Studio C++ Build Tools

```powershell
npm install
npm run tauri dev
```

## 提交前检查

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

涉及 Windows 安装流程时，再执行：

```powershell
npm run tauri build
```

## 提交建议

1. 一个 Pull Request 只解决一个明确问题。
2. 修改网络协议、加密、密钥存储时补充测试和兼容说明。
3. 不要提交剪贴板内容、设备配置、日志、密钥或包含真实设备信息的截图。
4. UI 文案优先保持简洁，并同时检查 Windows 缩放与窄窗口表现。

发现安全问题时不要公开提交 Issue，请按 [SECURITY.md](SECURITY.md) 中的方式报告。
