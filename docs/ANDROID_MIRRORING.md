# 安卓投屏实验版

`feature/android-mirroring-scrcpy` 分支先提供桌面端安卓投屏，不开发安卓配套 App。

## 当前范围

- LanClip 增加“安卓投屏”页面。
- 自动检测本机是否可以找到 `scrcpy` 和 `adb`。
- 地址留空时启动已通过 USB 授权的安卓设备。
- 填入安卓设备 IP 和端口时，尝试通过 ADB TCP/IP 启动无线投屏。
- 可选择“仅查看”或“允许鼠标键盘控制”。
- 可在标准触控和 UHID 兼容输入模式之间切换。
- 投屏窗口由 scrcpy 独立管理，LanClip 关闭主窗口不会影响剪贴板服务。

当前版本还没有把安卓设备接入 LanClip 的 6 位配对码体系。原因是没有安卓端 App 时，手机无法运行 LanClip 的发现、配对和身份协议；本阶段使用 ADB 的设备授权作为连接前提。

## 安装 scrcpy

请从 [Genymobile/scrcpy 官方仓库](https://github.com/Genymobile/scrcpy) 获取适合 Windows 或 macOS 的版本，并确保 `scrcpy`、`adb` 可以在终端中直接运行。

验证安装：

```powershell
scrcpy --version
adb version
```

如果 LanClip 显示“尚未找到 scrcpy”，请把 scrcpy 所在目录加入系统 `PATH`，然后完全退出并重新打开 LanClip。

## USB 投屏

1. 在安卓手机打开开发者选项和 USB 调试。
2. 使用 USB 线连接电脑。
3. 手机上出现 RSA 授权提示时选择允许。
4. 在 LanClip 的“安卓投屏”页面保持 IP 地址为空，点击“开始投屏”。

小米、红米和 POCO 设备如果画面能显示但鼠标不能点击，请在开发者选项中开启“USB 调试（安全设置）”或“允许通过 USB 调试模拟输入”，然后重启手机。也可以在 LanClip 中切换到 UHID 兼容模式。scrcpy 官方 FAQ 对这类设备的控制要求有明确说明。

## 局域网投屏

1. 让手机和电脑连接同一个局域网。
2. Android 11 及以上打开“无线调试”。无线调试页面会显示“配对端口”和“连接端口”，两者不是同一个端口。
3. 在终端完成配对和连接：

```powershell
adb pair 手机IP:配对端口
adb connect 手机IP:连接端口
```

4. 在 LanClip 中填写手机 IP 和“连接端口”；如果使用 USB 执行过 `adb tcpip 5555`，则填写 `5555`。
5. 点击“开始投屏”。

无线 ADB 可能在手机重启或关闭无线调试后失效，需要重新授权或连接。这是 Android 调试通道的系统限制，不是 LanClip 的配对状态。

如果 USB 和无线 ADB 同时连接，LanClip 留空 IP 时会明确选择 USB 设备；填写 IP 时会明确选择无线设备，避免 scrcpy 报告多个设备。

## 安全边界

- LanClip 不会静默打开 Android 控制权限。
- “允许鼠标键盘控制”关闭时，scrcpy 以仅查看模式启动。
- 只有已经通过 USB/ADB 明确授权的设备才能被投屏。
- 当前实验版不包含音频转发、录屏、文件传输和安卓剪贴板同步。

## 后续方向

验证课堂场景稳定后，再决定是否增加 ADB 设备登记、二维码绑定，或开发原生安卓配套 App，以便真正接入 LanClip 的可信设备体系。
