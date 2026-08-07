import { FormEvent, ReactNode, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type PairingSession = { code: string; expiresAt: number };
type Peer = {
  deviceId: string;
  deviceName: string;
  online: boolean;
  address?: string;
  pairedAt: number;
  lastSeen?: number;
};
type DiscoveredDevice = {
  deviceId: string;
  deviceName: string;
  address: string;
  port: number;
  pairing: boolean;
  lastSeen: number;
};
type Transfer = {
  id: string;
  direction: "sent" | "received";
  peerName: string;
  contentType: "text" | "image";
  preview: string;
  byteSize: number;
  createdAt: number;
  success: boolean;
  thumbnailDataUrl?: string;
};
type Snapshot = {
  deviceId: string;
  deviceName: string;
  syncEnabled: boolean;
  pairing?: PairingSession;
  peers: Peer[];
  discovered: DiscoveredDevice[];
  transfers: Transfer[];
};
type SystemStatus = {
  platform: "windows" | "macos" | "unsupported";
  autostartEnabled: boolean;
  firewallReady: boolean;
  secureStorage: boolean;
  installedMode: boolean;
};
type MirrorStatus = {
  available: boolean;
  scrcpyPath?: string;
  scrcpyVersion?: string;
  adbAvailable: boolean;
  running: boolean;
};
type MirrorDevice = {
  deviceId: string;
  deviceName: string;
  online: boolean;
  address?: string;
  port?: number;
  transport: string;
  bound: boolean;
  lastSeen: number;
};
type MirrorMouseMode = "sdk" | "uhid";

const APP_VERSION = "0.2.3";

const demoSnapshot: Snapshot = {
  deviceId: "preview-device",
  deviceName: "COURSE-STUDIO",
  syncEnabled: true,
  peers: [],
  discovered: [],
  transfers: [],
};

const isTauri = () => "__TAURI_INTERNALS__" in window;

function Icon({ name, size = 20 }: { name: string; size?: number }) {
  const paths: Record<string, ReactNode> = {
    clipboard: <><rect x="6" y="4" width="12" height="17" rx="2"/><path d="M9 4.5V3.8A1.8 1.8 0 0 1 10.8 2h2.4A1.8 1.8 0 0 1 15 3.8v.7"/><path d="M9 10h6M9 14h6"/></>,
    home: <><path d="m3 11 9-8 9 8"/><path d="M5.5 9.5V21h13V9.5M9.5 21v-6h5v6"/></>,
    devices: <><rect x="3" y="4" width="13" height="10" rx="2"/><path d="M8 18h3M9.5 14v4"/><rect x="17" y="8" width="4" height="10" rx="1"/></>,
    activity: <path d="M3 12h4l2.2-7 4.2 14 2.3-7H21"/>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.1A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3v-4h.1A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3h4v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9c.16.4.38.74.7 1 .3.25.7.4 1.1.4h.1v4h-.1a1.7 1.7 0 0 0-1.8.6Z"/></>,
    shield: <path d="M12 22s8-3.7 8-10V5l-8-3-8 3v7c0 6.3 8 10 8 10Z"/>,
    plus: <path d="M12 5v14M5 12h14"/>,
    monitor: <><rect x="2.5" y="4" width="19" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></>,
    phone: <><rect x="7" y="2.5" width="10" height="19" rx="2"/><path d="M10 5h4M11 18.5h2"/></>,
    copy: <><rect x="8" y="8" width="11" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h2"/></>,
    arrowUp: <path d="m6 15 6-6 6 6"/>,
    arrowDown: <path d="m6 9 6 6 6-6"/>,
    text: <><path d="M5 5h14M12 5v14M8 19h8"/></>,
    image: <><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 15-5-5L5 20"/></>,
    lock: <><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></>,
    wifi: <><path d="M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0"/><circle cx="12" cy="20" r=".7" fill="currentColor"/></>,
    more: <><circle cx="5" cy="12" r="1" fill="currentColor"/><circle cx="12" cy="12" r="1" fill="currentColor"/><circle cx="19" cy="12" r="1" fill="currentColor"/></>,
    close: <path d="m6 6 12 12M18 6 6 18"/>,
    check: <path d="m5 12 4 4L19 6"/>,
  };
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>(demoSnapshot);
  const [page, setPage] = useState<"overview" | "devices" | "activity" | "mirror" | "settings">("overview");
  const [busy, setBusy] = useState<string>();
  const [pairTarget, setPairTarget] = useState<DiscoveredDevice>();
  const [pairCode, setPairCode] = useState("");
  const [renameOpen, setRenameOpen] = useState(false);
  const [deviceName, setDeviceName] = useState("");
  const [error, setError] = useState<string>();
  const [systemStatus, setSystemStatus] = useState<SystemStatus>();
  const [mirrorStatus, setMirrorStatus] = useState<MirrorStatus>();
  const [mirrorDevices, setMirrorDevices] = useState<MirrorDevice[]>([]);
  const [mirrorAddress, setMirrorAddress] = useState("");
  const [mirrorPort, setMirrorPort] = useState("5555");
  const [mirrorControl, setMirrorControl] = useState(true);
  const [mirrorMouseMode, setMirrorMouseMode] = useState<MirrorMouseMode>("sdk");
  const [mirrorMaxFps, setMirrorMaxFps] = useState(60);
  const [mirrorAudio, setMirrorAudio] = useState(true);
  const [mirrorScreenOff, setMirrorScreenOff] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setSnapshot(await invoke<Snapshot>("get_snapshot"));
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const refreshSystem = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setSystemStatus(await invoke<SystemStatus>("get_system_status"));
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const refreshMirror = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setMirrorStatus(await invoke<MirrorStatus>("get_mirror_status"));
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const refreshMirrorDevices = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setMirrorDevices(await invoke<MirrorDevice[]>("get_mirror_devices"));
    } catch {
      // ADB may be unavailable while scrcpy is being installed. Keep the page
      // usable and let the status card explain the missing dependency.
    }
  }, []);

  useEffect(() => {
    refresh();
    refreshSystem();
    refreshMirror();
    const timer = window.setInterval(() => { refresh(); refreshMirror(); }, 1200);
    return () => window.clearInterval(timer);
  }, [refresh, refreshSystem, refreshMirror]);

  useEffect(() => {
    if (page !== "mirror") return;
    refreshMirrorDevices();
    const timer = window.setInterval(refreshMirrorDevices, 4000);
    return () => window.clearInterval(timer);
  }, [page, refreshMirrorDevices]);

  useEffect(() => setDeviceName(snapshot.deviceName), [snapshot.deviceName]);
  useEffect(() => {
    if (!error) return;
    const timer = window.setTimeout(() => setError(undefined), 4200);
    return () => window.clearTimeout(timer);
  }, [error]);

  const run = async (key: string, command: string, args?: Record<string, unknown>) => {
    if (!isTauri()) {
      setError("浏览器预览模式无法调用系统能力，请通过 Tauri 启动应用");
      return;
    }
    setBusy(key);
    try {
      await invoke(command, args);
      await refresh();
      if (command === "set_autostart" || command === "repair_firewall") await refreshSystem();
      if (command === "start_mirror" || command === "stop_mirror") await refreshMirror();
      if (command === "start_mirror" || command === "stop_mirror") await refreshMirrorDevices();
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setBusy(undefined);
    }
  };

  const onlineCount = snapshot.peers.filter((peer) => peer.online).length;
  const title = { overview: "概览", devices: "设备", activity: "传输记录", mirror: "安卓投屏", settings: "设置" }[page];

  const startPairing = () => run("pairing", "start_pairing").catch(() => undefined);
  const toggleSync = () => run("sync", "set_sync_enabled", { enabled: !snapshot.syncEnabled }).catch(() => undefined);
  const startMirror = (deviceId?: string) => run("mirror-start", "start_mirror", { deviceId: deviceId ?? null, address: mirrorAddress.trim() || null, port: Number(mirrorPort) || 5555, control: mirrorControl, mouseMode: mirrorMouseMode, maxFps: mirrorMaxFps, audio: mirrorAudio, screenOff: mirrorScreenOff }).catch(() => undefined);
  const stopMirror = () => run("mirror-stop", "stop_mirror").catch(() => undefined);

  const submitPair = async (event: FormEvent) => {
    event.preventDefault();
    if (!pairTarget) return;
    try {
      await run("connect", "pair_device", { deviceId: pairTarget.deviceId, code: pairCode });
      setPairTarget(undefined);
      setPairCode("");
    } catch { /* toast is handled by run */ }
  };

  const submitRename = async (event: FormEvent) => {
    event.preventDefault();
    try {
      await run("rename", "rename_device", { name: deviceName });
      setRenameOpen(false);
    } catch { /* toast is handled by run */ }
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark"><Icon name="clipboard" size={23}/></span>
          <span>LanClip</span>
        </div>

        <nav className="nav-list" aria-label="主导航">
          <NavButton active={page === "overview"} icon="home" label="概览" onClick={() => setPage("overview")}/>
          <NavButton active={page === "devices"} icon="devices" label="设备" badge={snapshot.peers.length || undefined} onClick={() => setPage("devices")}/>
          <NavButton active={page === "mirror"} icon="phone" label="安卓投屏" onClick={() => setPage("mirror")}/>
          <NavButton active={page === "activity"} icon="activity" label="传输记录" onClick={() => setPage("activity")}/>
          <NavButton active={page === "settings"} icon="settings" label="设置" onClick={() => setPage("settings")}/>
        </nav>

        <div className="sidebar-spacer"/>
        <div className="privacy-note">
          <Icon name="shield" size={18}/>
          <div><strong>仅在局域网内</strong><span>端到端加密传输</span></div>
        </div>
        <div className="device-identity">
          <span className="identity-avatar"><Icon name="monitor" size={18}/></span>
          <div><strong>{snapshot.deviceName}</strong><span>{snapshot.deviceId.slice(0, 8).toUpperCase()}</span></div>
          <button className="icon-button" aria-label="重命名设备" onClick={() => setRenameOpen(true)}><Icon name="more" size={18}/></button>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div><p className="eyebrow">LAN CLIPBOARD</p><h1>{title}</h1></div>
          <div className="topbar-actions">
            <span className={`connection-pill ${onlineCount ? "online" : ""}`}><i/>{onlineCount ? `${onlineCount} 台设备在线` : "等待设备连接"}</span>
            <button className="primary-button compact" onClick={startPairing} disabled={busy === "pairing"}><Icon name="plus" size={18}/>添加设备</button>
          </div>
        </header>

        {page === "overview" && <Overview snapshot={snapshot} onlineCount={onlineCount} busy={busy} onToggle={toggleSync} onStartPairing={startPairing} onPair={setPairTarget} onUnpair={(id) => run(`unpair-${id}`, "unpair_device", { deviceId: id }).catch(() => undefined)} />}
        {page === "devices" && <DevicesPage snapshot={snapshot} busy={busy} onStartPairing={startPairing} onPair={setPairTarget} onUnpair={(id) => run(`unpair-${id}`, "unpair_device", { deviceId: id }).catch(() => undefined)} />}
        {page === "mirror" && <MirrorPage platform={systemStatus?.platform} status={mirrorStatus} devices={mirrorDevices} busy={busy} address={mirrorAddress} setAddress={setMirrorAddress} port={mirrorPort} setPort={setMirrorPort} control={mirrorControl} setControl={setMirrorControl} mouseMode={mirrorMouseMode} setMouseMode={setMirrorMouseMode} maxFps={mirrorMaxFps} setMaxFps={setMirrorMaxFps} audio={mirrorAudio} setAudio={setMirrorAudio} screenOff={mirrorScreenOff} setScreenOff={setMirrorScreenOff} onRefreshDevices={refreshMirrorDevices} onStart={startMirror} onStop={stopMirror} onUnbind={(id) => run(`mirror-unbind-${id}`, "unbind_mirror_device", { deviceId: id }).then(refreshMirrorDevices).catch(() => undefined)} />}
        {page === "activity" && <ActivityPage transfers={snapshot.transfers} onClear={() => run("clear", "clear_history").catch(() => undefined)}/>}
        {page === "settings" && <SettingsPage snapshot={snapshot} systemStatus={systemStatus} busy={busy} onToggle={toggleSync} onRename={() => setRenameOpen(true)} onAutostart={() => run("autostart", "set_autostart", { enabled: !systemStatus?.autostartEnabled }).catch(() => undefined)} onFirewall={() => run("firewall", "repair_firewall").catch(() => undefined)}/>}
      </main>

      {snapshot.pairing && <PairingCodeModal session={snapshot.pairing} onClose={() => run("cancel", "cancel_pairing").catch(() => undefined)}/>}
      {pairTarget && <EnterCodeModal target={pairTarget} code={pairCode} setCode={setPairCode} busy={busy === "connect"} onClose={() => setPairTarget(undefined)} onSubmit={submitPair}/>}
      {renameOpen && <RenameModal value={deviceName} setValue={setDeviceName} busy={busy === "rename"} onClose={() => setRenameOpen(false)} onSubmit={submitRename}/>}
      {error && <div className="toast"><span>!</span>{error}</div>}
    </div>
  );
}

function NavButton({ active, icon, label, badge, onClick }: { active: boolean; icon: string; label: string; badge?: number; onClick: () => void }) {
  return <button className={`nav-button ${active ? "active" : ""}`} onClick={onClick}><Icon name={icon}/><span>{label}</span>{badge && <b>{badge}</b>}</button>;
}

function Overview({ snapshot, onlineCount, busy, onToggle, onStartPairing, onPair, onUnpair }: { snapshot: Snapshot; onlineCount: number; busy?: string; onToggle: () => void; onStartPairing: () => void; onPair: (d: DiscoveredDevice) => void; onUnpair: (id: string) => void }) {
  return <div className="page-stack">
    <section className={`sync-hero ${snapshot.syncEnabled ? "active" : "paused"}`}>
      <div className="sync-visual"><span className="orbit orbit-one"/><span className="orbit orbit-two"/><span className="sync-core"><Icon name={snapshot.syncEnabled ? "check" : "clipboard"} size={30}/></span></div>
      <div className="sync-copy"><span className="status-label">{snapshot.syncEnabled ? "同步守护中" : "同步已暂停"}</span><h2>{snapshot.syncEnabled ? "复制一下，另一台设备立即可用" : "剪贴板内容暂时不会离开这台设备"}</h2><p>{snapshot.syncEnabled ? `正在安全监听文字与图片${onlineCount ? `，并与 ${onlineCount} 台在线设备同步` : "，连接可信设备后即可自动同步"}。` : "恢复后将继续监听新的剪贴板内容。"}</p></div>
      <div className="toggle-block"><span>{snapshot.syncEnabled ? "已开启" : "已暂停"}</span><button className={`switch ${snapshot.syncEnabled ? "on" : ""}`} onClick={onToggle} disabled={busy === "sync"} aria-label="切换剪贴板同步"><i/></button></div>
    </section>

    <div className="section-heading"><div><h2>可信设备</h2><p>已配对设备会自动出现在这里</p></div><button className="text-button" onClick={onStartPairing}><Icon name="plus" size={17}/>允许新设备</button></div>
    {snapshot.peers.length ? <div className="device-grid">{snapshot.peers.map((peer) => <PeerCard key={peer.deviceId} peer={peer} busy={busy === `unpair-${peer.deviceId}`} onUnpair={() => onUnpair(peer.deviceId)}/>)}</div> : <EmptyDevices onStart={onStartPairing}/>}

    {!!snapshot.discovered.length && <><div className="section-heading discovered-heading"><div><h2>附近设备</h2><p>同一局域网中正在运行 LanClip</p></div></div><div className="discovered-list">{snapshot.discovered.map((device) => <div className="discovered-row" key={device.deviceId}><span className="device-icon"><Icon name="monitor"/></span><div><strong>{device.deviceName}</strong><span>{device.address} · {device.pairing ? "正在等待配对" : "未开放配对"}</span></div><button className="secondary-button" disabled={!device.pairing} onClick={() => onPair(device)}>输入配对码</button></div>)}</div></>}
    <RecentTransfers transfers={snapshot.transfers.slice(0, 5)}/>
  </div>;
}

function DevicesPage({ snapshot, busy, onStartPairing, onPair, onUnpair }: { snapshot: Snapshot; busy?: string; onStartPairing: () => void; onPair: (d: DiscoveredDevice) => void; onUnpair: (id: string) => void }) {
  return <div className="page-stack"><div className="info-banner"><span><Icon name="lock"/></span><div><strong>每台设备使用独立密钥</strong><p>解除绑定后，该设备将无法再读取或发送任何剪贴板内容。</p></div></div><div className="section-heading"><div><h2>已配对 · {snapshot.peers.length}</h2><p>只有这些设备可以参与自动同步</p></div><button className="primary-button compact" onClick={onStartPairing}><Icon name="plus" size={17}/>允许新设备</button></div>{snapshot.peers.length ? <div className="device-grid">{snapshot.peers.map((peer) => <PeerCard key={peer.deviceId} peer={peer} busy={busy === `unpair-${peer.deviceId}`} onUnpair={() => onUnpair(peer.deviceId)}/>)}</div> : <EmptyDevices onStart={onStartPairing}/>}<div className="section-heading discovered-heading"><div><h2>局域网发现</h2><p>{snapshot.discovered.length ? `发现 ${snapshot.discovered.length} 台未绑定设备` : "暂未发现其他设备"}</p></div></div>{snapshot.discovered.map((device) => <div className="discovered-row" key={device.deviceId}><span className="device-icon"><Icon name="monitor"/></span><div><strong>{device.deviceName}</strong><span>{device.address} · {device.pairing ? "可配对" : "等待对方开放配对"}</span></div><button className="secondary-button" disabled={!device.pairing} onClick={() => onPair(device)}>连接</button></div>)}</div>;
}

function PeerCard({ peer, busy, onUnpair }: { peer: Peer; busy: boolean; onUnpair: () => void }) {
  const [menu, setMenu] = useState(false);
  return <article className="peer-card"><div className="peer-top"><span className="device-icon large"><Icon name="monitor" size={24}/><i className={peer.online ? "online" : ""}/></span><button className="icon-button" onClick={() => setMenu(!menu)}><Icon name="more"/>{menu && <span className="popover" onClick={(e) => {e.stopPropagation(); onUnpair();}}>{busy ? "正在解除…" : "解除绑定"}</span>}</button></div><h3>{peer.deviceName}</h3><p>{peer.online ? peer.address : "当前离线"}</p><div className="peer-state"><span className={peer.online ? "online" : ""}><i/>{peer.online ? "在线 · 自动同步" : "离线"}</span><Icon name="lock" size={15}/></div></article>;
}

function MirrorPage({ platform, status, devices, busy, address, setAddress, port, setPort, control, setControl, mouseMode, setMouseMode, maxFps, setMaxFps, audio, setAudio, screenOff, setScreenOff, onRefreshDevices, onStart, onStop, onUnbind }: {
  platform?: SystemStatus["platform"];
  status?: MirrorStatus;
  devices: MirrorDevice[];
  busy?: string;
  address: string;
  setAddress: (value: string) => void;
  port: string;
  setPort: (value: string) => void;
  control: boolean;
  setControl: (value: boolean) => void;
  mouseMode: MirrorMouseMode;
  setMouseMode: (value: MirrorMouseMode) => void;
  maxFps: number;
  setMaxFps: (value: number) => void;
  audio: boolean;
  setAudio: (value: boolean) => void;
  screenOff: boolean;
  setScreenOff: (value: boolean) => void;
  onRefreshDevices: () => void;
  onStart: (deviceId?: string) => void;
  onStop: () => void;
  onUnbind: (deviceId: string) => void;
}) {
  const ready = !!status?.available;
  const running = !!status?.running;
  const isMac = platform === "macos";
  const [wirelessOpen, setWirelessOpen] = useState(false);
  const [guideOpen, setGuideOpen] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem("lanclip.mirrorGuideSeen") !== "1";
  });
  const closeGuide = () => {
    window.localStorage.setItem("lanclip.mirrorGuideSeen", "1");
    setGuideOpen(false);
  };

  return <div className="page-stack mirror-page">
    {guideOpen && <MirrorGuide onClose={closeGuide}/>}
    <section className={`mirror-hero ${running ? "running" : ""}`}>
      <div className="mirror-hero-icon"><Icon name="phone" size={30}/></div>
      <div><span className="status-label">Windows / macOS 桌面端</span><h2>把安卓屏幕带到电脑上</h2><p>LanClip 负责设备记忆和会话管理，scrcpy 负责低延迟投屏、音频转发与鼠标键盘控制。</p></div>
      <span className={`mirror-status ${running ? "on" : ""}`}><i/>{running ? "投屏中" : "未启动"}</span>
    </section>

    {status && !ready && <section className="info-banner mirror-warning"><span><Icon name="phone"/></span><div><strong>投屏组件不可用</strong><p>{isMac ? "安装包会内置 adb 与 scrcpy。如果这里仍提示不可用，请重新安装最新版 LanClip。" : "请安装包含 scrcpy 的最新版 LanClip，或把官方 scrcpy 加入系统 PATH 后重启。"}</p><a href="https://github.com/Genymobile/scrcpy" target="_blank" rel="noreferrer">查看 scrcpy 官方说明</a></div></section>}
    {ready && !status?.adbAvailable && <section className="info-banner mirror-warning"><span><Icon name="phone"/></span><div><strong>ADB 不可用</strong><p>{isMac ? "安装包会内置 Android platform-tools。如果这里仍提示不可用，请重新安装最新版 LanClip。" : "请安装包含 adb 的最新版 LanClip，或把 Android platform-tools 加入系统 PATH 后重启。"}</p><a href="https://developer.android.com/tools/adb" target="_blank" rel="noreferrer">查看 ADB 官方说明</a></div></section>}

    <section className="settings-card mirror-devices-card">
      <div className="section-heading"><div><h2>已发现的安卓设备</h2><p>USB 调试授权后会自动出现；首次成功投屏后会记住这台设备</p></div><div className="section-actions"><button className="secondary-button" onClick={() => setGuideOpen(true)}>查看引导</button><button className="secondary-button" onClick={onRefreshDevices} disabled={busy === "mirror-start"}>刷新设备</button></div></div>
      {devices.length ? <div className="mirror-device-list">{devices.map((device) => <div className={`mirror-device-row ${device.online ? "online" : "offline"}`} key={device.deviceId}>
        <span className="device-icon large"><Icon name="phone" size={21}/><i className={device.online ? "online" : ""}/></span>
        <div className="mirror-device-copy"><strong>{device.deviceName}</strong><span>{device.transport === "wifi" ? "无线 ADB" : "USB"}{device.address ? ` · ${device.address}${device.port ? `:${device.port}` : ""}` : ""}</span><small>{device.bound ? "已绑定 · " : "附近设备 · 投屏后自动绑定 · "}{device.online ? "在线" : "当前离线"}</small></div>
        <div className="mirror-device-actions"><button className="primary-button compact" onClick={() => onStart(device.deviceId)} disabled={!ready || !device.online || running || busy === "mirror-start"}>{busy === "mirror-start" ? "启动中…" : "一键投屏"}</button>{device.bound && <button className="text-button muted" onClick={() => onUnbind(device.deviceId)} disabled={busy === `mirror-unbind-${device.deviceId}`}>{busy === `mirror-unbind-${device.deviceId}` ? "解除中…" : "解除绑定"}</button>}</div>
      </div>)}</div> : <div className="mirror-device-empty"><Icon name="phone" size={20}/><span>暂未发现 ADB 设备。请用 USB 连接手机，开启 USB 调试，并在手机上允许这台电脑调试。</span></div>}
    </section>

    <div className="mirror-grid">
      <section className="settings-card mirror-card">
        <div className="section-heading"><div><h2>投屏设置</h2><p>默认使用 USB 授权设备；无需填写 IP 和端口</p></div>{status?.adbAvailable && <span className="health-badge ok"><i/>ADB 可用</span>}</div>
        <div className="mirror-usb-start">
          <span><Icon name="phone" size={20}/></span>
          <div><strong>USB 投屏</strong><p>连接手机并允许 USB 调试后，直接点击开始投屏。若上方已出现设备，也可以点“一键投屏”。</p></div>
        </div>
        <label className="mirror-control"><input type="checkbox" checked={control} onChange={(event) => setControl(event.target.checked)} disabled={running}/><span><strong>允许鼠标键盘控制</strong><small>关闭后仅投屏查看，不会向手机发送操作</small></span></label>
        <label className="mirror-control"><input type="checkbox" checked={audio} onChange={(event) => setAudio(event.target.checked)} disabled={running}/><span><strong>电脑播放安卓音频</strong><small>Android 11+ 会将声音转发到电脑，并关闭手机扬声器播放</small></span></label>
        <label className="mirror-control"><input type="checkbox" checked={screenOff} onChange={(event) => setScreenOff(event.target.checked)} disabled={running}/><span><strong>投屏后关闭手机屏幕</strong><small>保持投屏与控制，但关闭手机显示屏；这不是 Android 锁屏</small></span></label>
        {control && <label className="mirror-field mirror-mode-field"><span>输入控制模式</span><select value={mouseMode} onChange={(event) => setMouseMode(event.target.value as MirrorMouseMode)} disabled={running}><option value="sdk">标准触控（推荐）</option><option value="uhid">UHID 兼容模式（小米设备可尝试）</option></select><small>{mouseMode === "uhid" ? "鼠标会被捕获；按 Alt 或 Super 可释放鼠标。" : "如果小米/红米设备无法点击，请开启 USB 调试（安全设置）或改用 UHID。"}</small></label>}
        <label className="mirror-field mirror-mode-field"><span>投屏帧率上限</span><select value={maxFps} onChange={(event) => setMaxFps(Number(event.target.value))} disabled={running}><option value={30}>30 FPS · 稳定省资源</option><option value={60}>60 FPS · 推荐</option><option value={90}>90 FPS</option><option value={120}>120 FPS</option><option value={165}>165 FPS · 设备支持时</option></select><small>这是视频采集上限，不等于实际帧率；实际效果取决于手机刷新率、编码器和 USB/无线带宽。</small></label>
        <button className="text-button mirror-advanced-toggle" onClick={() => setWirelessOpen(!wirelessOpen)}>{wirelessOpen ? "收起无线/手动连接" : "无线调试或手动 IP 连接"}</button>
        {wirelessOpen && <div className="mirror-wireless-panel">
          <div className="mirror-form">
            <label className="mirror-field"><span>安卓设备 IP</span><input value={address} onChange={(event) => setAddress(event.target.value)} placeholder="例如 192.168.1.25" disabled={running}/></label>
            <label className="mirror-field port-field"><span>连接端口</span><input value={port} onChange={(event) => setPort(event.target.value.replace(/\D/g, "").slice(0, 5))} inputMode="numeric" placeholder="5555" disabled={running}/></label>
          </div>
          <p>仅在 Android 11+ 无线调试已配对、但设备没有自动出现在列表中时使用。这里填写的是“连接端口”，不是“配对端口”。</p>
        </div>}
        <div className="mirror-actions"><button className="primary-button" onClick={() => onStart()} disabled={!ready || running || busy === "mirror-start"}><Icon name="phone" size={17}/>{busy === "mirror-start" ? "正在启动…" : "开始投屏"}</button><button className="secondary-button" onClick={onStop} disabled={!running || busy === "mirror-stop"}>{busy === "mirror-stop" ? "正在停止…" : "停止投屏"}</button></div>
        {status?.scrcpyVersion && <p className="mirror-version">检测到 {status.scrcpyVersion}</p>}
      </section>

      <section className="settings-card mirror-card">
        <h2>首次连接步骤</h2>
        <ol className="mirror-steps"><li>在安卓手机打开开发者选项和 USB 调试。</li><li>用 USB 线连接电脑，手机弹出授权提示时选择允许。</li><li>回到本页，确认上方设备列表出现手机，或直接点击“开始投屏”。</li><li>小米/红米/POCO 若需要鼠标键盘控制，开启“USB 调试（安全设置）/允许通过 USB 调试模拟输入”，开启后重启手机。</li><li>无线调试是进阶用法；只有 USB 不方便或已经完成无线 ADB 配对时，才需要展开手动 IP/端口。</li></ol>
        <div className="mirror-note"><Icon name="shield" size={16}/><span>当前版本不安装安卓 App，也不会把控制权限授予未授权的设备。投屏窗口由 scrcpy 管理。</span></div>
      </section>
    </div>
  </div>;
}

function MirrorGuide({ onClose }: { onClose: () => void }) {
  return <div className="modal-backdrop">
    <section className="modal mirror-guide" role="dialog" aria-modal="true" aria-labelledby="mirror-guide-title">
      <button className="modal-close" onClick={onClose} aria-label="关闭引导"><Icon name="close" size={18}/></button>
      <div className="modal-icon"><Icon name="phone" size={26}/></div>
      <h2 id="mirror-guide-title">首次安卓投屏</h2>
      <p>LanClip 已内置 adb 和 scrcpy。大多数同事只需要插上 USB、允许调试，然后点击开始投屏。</p>
      <div className="guide-step-list">
        <div><b>1</b><span><strong>打开 USB 调试</strong><small>手机进入开发者选项，开启 USB 调试。</small></span></div>
        <div><b>2</b><span><strong>连接并授权</strong><small>用 USB 连接电脑，手机弹出授权时选择允许。</small></span></div>
        <div><b>3</b><span><strong>开始投屏</strong><small>设备出现后点一键投屏；也可以直接点开始投屏。</small></span></div>
        <div><b>4</b><span><strong>控制权限</strong><small>小米/红米/POCO 要控制点击，请额外开启 USB 调试（安全设置）并重启。</small></span></div>
      </div>
      <button className="primary-button modal-submit" onClick={onClose}>知道了</button>
    </section>
  </div>;
}

function EmptyDevices({ onStart }: { onStart: () => void }) {
  return <div className="empty-card"><div className="empty-art"><span><Icon name="monitor" size={26}/></span><i/><span><Icon name="monitor" size={26}/></span></div><div><h3>还没有可信设备</h3><p>在另一台电脑打开 LanClip，然后用一次性配对码安全连接。</p></div><button className="primary-button" onClick={onStart}><Icon name="plus" size={18}/>开始配对</button></div>;
}

function RecentTransfers({ transfers }: { transfers: Transfer[] }) {
  return <section className="recent-section"><div className="section-heading"><div><h2>最近传输</h2><p>仅显示内容摘要，不保存剪贴板原文</p></div></div>{transfers.length ? <div className="transfer-list">{transfers.map((item) => <TransferRow key={item.id} item={item}/>)}</div> : <div className="quiet-empty"><Icon name="activity" size={24}/><span>完成首次同步后，记录会出现在这里</span></div>}</section>;
}

function TransferRow({ item }: { item: Transfer }) {
  const hasThumbnail = item.contentType === "image" && !!item.thumbnailDataUrl;
  return <div className={`transfer-row ${hasThumbnail ? "has-thumbnail" : ""}`}><span className={`transfer-icon ${item.direction}`}><Icon name={item.direction === "sent" ? "arrowUp" : "arrowDown"} size={18}/></span>{hasThumbnail ? <img className="transfer-thumbnail" src={item.thumbnailDataUrl} alt={item.preview}/> : <span className="content-icon"><Icon name={item.contentType === "text" ? "text" : "image"} size={18}/></span>}<div><strong>{item.preview}</strong><span>{item.direction === "sent" ? `发送至 ${item.peerName}` : `来自 ${item.peerName}`} · {formatBytes(item.byteSize)}</span></div><time>{formatTime(item.createdAt)}</time>{!item.success && <b className="failed">失败</b>}</div>;
}

function ActivityPage({ transfers, onClear }: { transfers: Transfer[]; onClear: () => void }) {
  return <div className="page-stack"><div className="section-heading"><div><h2>本次运行的传输</h2><p>退出应用后自动清空，不在磁盘保存内容记录</p></div>{!!transfers.length && <button className="text-button muted" onClick={onClear}>清空记录</button>}</div>{transfers.length ? <div className="transfer-list full">{transfers.map((item) => <TransferRow key={item.id} item={item}/>)}</div> : <div className="large-empty"><span><Icon name="activity" size={30}/></span><h3>还没有传输记录</h3><p>复制文字或截图后，LanClip 会自动将它发送给在线的可信设备。</p></div>}</div>;
}

function SettingsPage({ snapshot, systemStatus, busy, onToggle, onRename, onAutostart, onFirewall }: { snapshot: Snapshot; systemStatus?: SystemStatus; busy?: string; onToggle: () => void; onRename: () => void; onAutostart: () => void; onFirewall: () => void }) {
  const platform = systemStatus?.platform ?? "windows";
  const isMac = platform === "macos";
  const autostartLabel = systemStatus ? (systemStatus.autostartEnabled ? "已开启" : "已关闭") : "检测中";
  return <div className="settings-layout"><section className="settings-card"><h2>同步偏好</h2><SettingRow title="自动同步剪贴板" detail="监测并发送新复制的文字和图片"><button className={`switch ${snapshot.syncEnabled ? "on" : ""}`} onClick={onToggle} disabled={busy === "sync"} aria-label="切换自动同步剪贴板"><i/></button></SettingRow><SettingRow title="开机自动启动" detail={`登录系统后静默启动到${isMac ? "菜单栏" : "系统托盘"}`}><span className="setting-action switch-action"><span className="setting-value">{busy === "autostart" ? "正在更新" : autostartLabel}</span><button className={`switch ${systemStatus?.autostartEnabled ? "on" : ""}`} onClick={onAutostart} disabled={!systemStatus || busy === "autostart"} aria-label="切换开机自动启动"><i/></button></span></SettingRow><SettingRow title="关闭窗口行为" detail={`关闭主窗口后继续在${isMac ? "菜单栏" : "系统托盘"}同步`}><span className="health-badge ok"><i/>已启用</span></SettingRow><SettingRow title="图片大小限制" detail="超过限制的图片不会发送"><span className="setting-value">20 MB</span></SettingRow></section><section className="settings-card"><h2>安全与网络</h2><SettingRow title="系统安全凭据" detail={`设备密钥存放在 ${isMac ? "macOS Keychain" : "Windows Credential Manager"}`}><span className={`health-badge ${systemStatus?.secureStorage ? "ok" : "warn"}`}><i/>{systemStatus?.secureStorage ? "已保护" : "不可用"}</span></SettingRow>{isMac ? <SettingRow title="本地网络权限" detail="首次发现局域网设备时请允许 macOS 本地网络访问"><span className="health-badge ok"><i/>由系统管理</span></SettingRow> : <SettingRow title="Windows 防火墙" detail="允许可信设备通过局域网连接本机"><span className="setting-action">{systemStatus?.firewallReady ? <span className="health-badge ok"><i/>已允许</span> : <button className="secondary-button" onClick={onFirewall} disabled={busy === "firewall"}>{busy === "firewall" ? "等待授权…" : "修复权限"}</button>}</span></SettingRow>}<SettingRow title="安装状态" detail={isMac ? "开发运行和正式安装都支持菜单栏常驻" : "安装版可自动维护防火墙规则和快捷方式"}><span className={`health-badge ${systemStatus?.installedMode ? "ok" : "warn"}`}><i/>{systemStatus?.installedMode ? "已安装" : "开发运行"}</span></SettingRow></section><section className="settings-card"><h2>本机身份</h2><SettingRow title={snapshot.deviceName} detail={`设备 ID · ${snapshot.deviceId}`}><button className="secondary-button" onClick={onRename}>重命名</button></SettingRow></section><section className="settings-card about"><span className="brand-mark"><Icon name="clipboard"/></span><div><h2>LanClip {APP_VERSION}</h2><p>Windows 优先，已支持 Windows ↔ macOS 双向同步</p></div></section></div>;
}

function SettingRow({ title, detail, children }: { title: string; detail: string; children: ReactNode }) { return <div className="setting-row"><div><strong>{title}</strong><span>{detail}</span></div>{children}</div>; }

function Modal({ children, onClose, wide = false }: { children: ReactNode; onClose: () => void; wide?: boolean }) { return <div className="modal-backdrop" onMouseDown={onClose}><div className={`modal ${wide ? "wide" : ""}`} onMouseDown={(e) => e.stopPropagation()}>{children}</div></div>; }

function PairingCodeModal({ session, onClose }: { session: PairingSession; onClose: () => void }) {
  const [remaining, setRemaining] = useState(Math.max(0, Math.ceil((session.expiresAt - Date.now()) / 1000)));
  useEffect(() => { const timer = window.setInterval(() => setRemaining(Math.max(0, Math.ceil((session.expiresAt - Date.now()) / 1000))), 500); return () => clearInterval(timer); }, [session.expiresAt]);
  return <Modal onClose={onClose}><button className="modal-close" onClick={onClose}><Icon name="close"/></button><span className="modal-icon"><Icon name="devices" size={27}/></span><h2>连接另一台设备</h2><p>在另一台电脑的“附近设备”中选择本机，然后输入以下配对码。</p><div className="pair-code" aria-label={`配对码 ${session.code}`}>{session.code.split("").map((digit, index) => <b key={index}>{digit}</b>)}</div><div className="expiry"><i style={{"--progress": `${remaining / 120 * 100}%`} as React.CSSProperties}/><span>{remaining ? `${remaining} 秒后失效` : "配对码已失效"}</span></div><div className="modal-security"><Icon name="lock" size={16}/>配对码只使用一次，不会作为长期密码保存</div></Modal>;
}

function EnterCodeModal({ target, code, setCode, busy, onClose, onSubmit }: { target: DiscoveredDevice; code: string; setCode: (v: string) => void; busy: boolean; onClose: () => void; onSubmit: (e: FormEvent) => void }) {
  return <Modal onClose={onClose}><button className="modal-close" onClick={onClose}><Icon name="close"/></button><span className="modal-icon"><Icon name="monitor" size={27}/></span><h2>连接 {target.deviceName}</h2><p>输入目标设备上显示的 6 位配对码，确认你正在连接正确的电脑。</p><form onSubmit={onSubmit}><input className="code-input" autoFocus inputMode="numeric" maxLength={6} value={code} onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))} placeholder="000000"/><button className="primary-button modal-submit" disabled={code.length !== 6 || busy}>{busy ? "正在安全连接…" : "确认连接"}</button></form><div className="modal-security"><Icon name="wifi" size={17}/>目标地址 {target.address} · 仅限当前局域网</div></Modal>;
}

function RenameModal({ value, setValue, busy, onClose, onSubmit }: { value: string; setValue: (v: string) => void; busy: boolean; onClose: () => void; onSubmit: (e: FormEvent) => void }) { return <Modal onClose={onClose}><button className="modal-close" onClick={onClose}><Icon name="close"/></button><span className="modal-icon"><Icon name="monitor" size={27}/></span><h2>重命名本机</h2><p>这个名称会显示在同一局域网内其他 LanClip 设备上。</p><form onSubmit={onSubmit}><input className="name-input" autoFocus maxLength={40} value={value} onChange={(e) => setValue(e.target.value)} /><button className="primary-button modal-submit" disabled={!value.trim() || busy}>{busy ? "正在保存…" : "保存名称"}</button></form></Modal>; }

function formatBytes(bytes: number) { if (bytes < 1024) return `${bytes} B`; if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`; return `${(bytes / 1024 / 1024).toFixed(1)} MB`; }
function formatTime(timestamp: number) { return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(timestamp); }

export default App;
