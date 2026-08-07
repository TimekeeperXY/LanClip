# Windows / macOS 安卓投屏

`feature/android-mirroring-scrcpy` 分支提供 Windows 和 macOS 桌面端安卓投屏，不开发安卓配套 App。

## 当前范围

- LanClip 增加“安卓投屏”页面。
- 自动检测本机是否可以找到 `scrcpy` 和 `adb`。
- 地址留空时启动已通过 USB 授权的安卓设备。
- 填入安卓设备 IP 和端口时，尝试通过 ADB TCP/IP 启动无线投屏。
- 可选择“仅查看”或“允许鼠标键盘控制”。
- 可在标准触控和 UHID 兼容输入模式之间切换。
- 可选择 30/60/90/120/165 FPS 的投屏帧率上限。
- Android 11+ 可将安卓音频转发到电脑扬声器，并关闭手机扬声器播放。
- 可选择投屏后关闭安卓显示屏，同时保持电脑端投屏与控制。
- 首次成功投屏后自动记住安卓设备；下次在“已记住的安卓设备”中点击“一键投屏”。
- 已配对的无线 ADB 设备通过 ADB mDNS 自动发现，IP 和无线连接端口变化时无需手填。
- 投屏窗口由 scrcpy 独立管理，LanClip 关闭主窗口不会影响剪贴板服务。

当前版本还没有把安卓设备接入 LanClip 的 6 位配对码体系。原因是没有安卓端 App 时，手机无法运行 LanClip 的发现、配对和身份协议；本阶段使用 ADB 的设备授权作为连接前提。

## 安装 scrcpy 与 ADB

请从 [Genymobile/scrcpy 官方仓库](https://github.com/Genymobile/scrcpy) 获取适合 Windows 或 macOS 的版本，并确保 `scrcpy`、`adb` 可以在终端中直接运行。

### macOS

推荐使用 Homebrew：

```bash
brew install scrcpy
brew install --cask android-platform-tools
```

Apple Silicon Mac 使用 aarch64 版本，Intel Mac 使用 x86_64 版本。若使用官方静态压缩包，请将解压目录加入 `PATH`；LanClip 也会自动检查 `/opt/homebrew/bin`、`/usr/local/bin` 和 `/opt/local/bin`。

首次运行 Mac 版 LanClip 时，如果系统询问本地网络权限，请选择允许；无线 ADB 的 mDNS 自动发现依赖该权限。USB 投屏仍需在手机上开启 USB 调试并接受 RSA 授权。

### Windows

可使用官方 Windows 压缩包，或通过 WinGet 安装 scrcpy；确保 `scrcpy.exe` 和 `adb.exe` 在 PATH 中。

验证安装：

```bash
scrcpy --version
adb version
```

如果 LanClip 显示“尚未找到 scrcpy”或“尚未找到 adb”，请把对应目录加入系统 `PATH`，然后完全退出并重新打开 LanClip。

## USB 投屏

1. 在安卓手机打开开发者选项和 USB 调试。
2. 使用 USB 线连接电脑。
3. 手机上出现 RSA 授权提示时选择允许。
4. 在 LanClip 的“安卓投屏”页面保持 IP 地址为空，点击“开始投屏”。

小米、红米和 POCO 设备如果画面能显示但鼠标不能点击，请在开发者选项中开启“USB 调试（安全设置）”或“允许通过 USB 调试模拟输入”，然后重启手机。也可以在 LanClip 中切换到 UHID 兼容模式。scrcpy 官方 FAQ 对这类设备的控制要求有明确说明。

## 局域网投屏与设备记忆

1. 让手机和电脑连接同一个局域网。
2. Android 11 及以上打开“无线调试”。无线调试页面会显示“配对端口”和“连接端口”，两者不是同一个端口。
3. 在终端完成配对和连接：

```bash
adb pair 手机IP:配对端口
adb connect 手机IP:连接端口
```

4. 在 LanClip 中填写手机 IP 和“连接端口”；如果使用 USB 执行过 `adb tcpip 5555`，则填写 `5555`。
5. 选择合适的投屏帧率并点击“开始投屏”。投屏成功后，LanClip 会保存 ADB 设备序列号和设备名称，而不是保存 IP 作为身份。
6. 下次打开“安卓投屏”页面，保持手机处于 USB 调试或无线调试可连接状态，LanClip 会通过 ADB mDNS 刷新在线设备；在设备列表点击“一键投屏”即可。

Android 11+ 的无线调试通常只需要首次执行 `adb pair`。完成配对后，Android 会通过 `_adb-tls-connect._tcp` mDNS 服务发布当前连接端口，LanClip 会自动尝试连接这些服务。若路由器隔离了 mDNS，仍可使用上方 IP/端口手动连接。

这里没有使用手机 MAC 地址作为绑定 ID：现代 Android 默认会对 Wi-Fi 使用随机化 MAC（且可按网络变化），因此它不适合跨网络记忆设备。ADB 序列号/系统序列号更适合作为本机设备身份；如果某些定制 ROM 不提供序列号，LanClip 会退回当前 ADB serial，并保留手动连接入口。

无线 ADB 可能在手机重启或关闭无线调试后失效，需要重新授权或连接。这是 Android 调试通道的系统限制，不是 LanClip 的配对状态。

如果 USB 和无线 ADB 同时连接同一台手机，设备列表会合并为一条记录，并优先使用无线 ADB；USB 会作为无线不可用时的备用连接。手动填写 IP 时会优先匹配对应的 ADB serial，避免 scrcpy 报告多个设备。

## 投屏帧率说明

LanClip 通过 scrcpy 的 `--max-fps` 设置视频采集帧率上限。165Hz 显示器可以显示高刷新率窗口，但不会把手机原本的 60Hz 画面变成 165Hz；手机屏幕刷新率、Android 版本、硬件编码器和连接带宽都会限制实际帧率。scrcpy 官方也说明帧率是可变的，只有屏幕内容变化时才会产生新帧。

一般建议 USB 使用 60 或 90 FPS；手机确实支持高刷新率且连接稳定时再尝试 120/165 FPS。无线投屏出现卡顿、延迟或画面丢帧时，优先降回 60 FPS，必要时再降低分辨率。

## 音频转发

勾选“电脑播放安卓音频”后，LanClip 使用 scrcpy 的 `output` 音频源：电脑播放安卓设备声音，同时 Android 端不再通过手机扬声器重复播放。scrcpy 官方支持 Android 11 及以上的音频转发；Android 12+ 通常可直接使用，Android 11 启动时需要保持手机屏幕解锁。Android 10 及以下无法捕获设备音频。[scrcpy 音频文档](https://github.com/Genymobile/scrcpy/blob/master/doc/audio.md)

如果不希望电脑播放声音，取消该选项即可恢复纯视频投屏。

## 手机息屏

勾选“投屏后关闭手机屏幕”后，LanClip 会使用 scrcpy 的 `--turn-screen-off`：手机显示屏关闭，但电脑端仍继续接收画面并可以操作。它是“关闭显示屏”，不等同于 Android 锁屏；如果需要真正的锁屏安全状态，锁屏后电脑端会看到锁屏界面，普通应用控制也会受到系统限制。scrcpy 官方也说明，手机实体电源键仍可能重新点亮屏幕，可在投屏窗口使用 `MOD`+`o` 切换息屏/亮屏。[scrcpy 设备控制文档](https://github.com/Genymobile/scrcpy/blob/master/doc/device.md)

不同 Android 厂商和系统版本对息屏控制的实现可能不同，尤其是部分 Android 14/15 或定制系统；如果画面冻结或手机自动亮屏，可取消该选项，或使用投屏窗口快捷键重新切换。

## macOS 验收流程

1. 在 Mac 上安装 scrcpy 与 ADB，并执行 `scrcpy --version`、`adb version`。
2. 用 USB 连接安卓手机，在手机上允许 USB 调试授权。
3. 启动 LanClip，进入“安卓投屏”，确认显示“ADB 可用”。
4. 先使用 USB 投屏验证画面、鼠标键盘、电脑音频和息屏开关。
5. 再完成 Android 11+ 无线调试配对，验证设备记忆和同一局域网自动发现。

## 安全边界

- LanClip 不会静默打开 Android 控制权限。
- “允许鼠标键盘控制”关闭时，scrcpy 以仅查看模式启动。
- 只有已经通过 USB/ADB 明确授权的设备才能被投屏。
- 当前版本不包含录屏、文件传输和安卓剪贴板同步；音频转发仅支持 Android 11+。

## 后续方向

验证课堂场景稳定后，再决定是否增加二维码绑定、设备别名管理，或开发原生安卓配套 App，以便真正接入 LanClip 的可信设备体系。
