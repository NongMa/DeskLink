//! scrcpy 投屏（分体式：scrcpy 原样运行 + 右侧原生侧边控制面板，按键走 adb keyevent）
//!
//! - scrcpy / platform-tools(adb) 以预编译二进制随服务打包在 vendor/ 下；
//!   发布形态 = <exe 目录>/vendor/…，开发形态（current_exe 在 target/ 下）向上逐级查找 windows-server/vendor/。
//! - 投屏只作用于「被投的手机」；语音键盘 / PC 指令仍作用于本机，两套通道互不影响。
//! - 关闭侧边面板即结束投屏（托盘退出时也会强制回收 scrcpy）。

use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Instant;

use winapi::shared::windef::RECT;
use winapi::um::winuser::{SystemParametersInfoW, SPI_GETWORKAREA};

pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 侧边控制面板宽度（像素）；面板与 scrcpy 窗口的横向位置都基于它计算
pub const PANEL_WIDTH: i32 = 236;

pub struct MirrorStatus {
    pub running: bool,
    pub serial: Option<String>,
    pub uptime_sec: Option<u64>,
}

struct MirrorInfo {
    child: Child,
    serial: String,
    started_at: Instant,
}

static MIRROR: Mutex<Option<MirrorInfo>> = Mutex::new(None);

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// vendor 目录定位：先 <exe 目录>/vendor（发布），再逐级向上找 windows-server/vendor / vendor（开发）
fn vendor_dir() -> Option<PathBuf> {
    let base = exe_dir();
    let direct = base.join("vendor");
    if direct.join("scrcpy").join("scrcpy.exe").exists() {
        return Some(direct);
    }
    let mut d = base;
    loop {
        for rel in ["windows-server/vendor", "vendor"] {
            let mut cand = d.clone();
            for part in rel.split('/') {
                cand.push(part);
            }
            if cand.join("scrcpy").join("scrcpy.exe").exists() {
                return Some(cand);
            }
        }
        match d.parent() {
            Some(p) => d = p.to_path_buf(),
            None => return None,
        }
    }
}

fn scrcpy_exe() -> anyhow::Result<PathBuf> {
    vendor_dir()
        .map(|v| v.join("scrcpy").join("scrcpy.exe"))
        .ok_or_else(|| anyhow::anyhow!("未找到 vendor/scrcpy/scrcpy.exe（请检查安装或重新打包）"))
}

pub(crate) fn adb_exe() -> anyhow::Result<PathBuf> {
    vendor_dir()
        .map(|v| v.join("platform-tools").join("adb.exe"))
        .ok_or_else(|| anyhow::anyhow!("未找到 vendor/platform-tools/adb.exe（请检查安装或重新打包）"))
}

#[derive(Clone, serde::Serialize)]
pub struct AdbDevice {
    pub serial: String,
    pub state: String,
    /// adb -l 输出里的 model: 字段（可能为空）
    pub model: String,
    /// serial 含 '.' 或 ':' 视为网络连接（WiFi/TCP）
    pub is_network: bool,
}

/// 解析 `adb devices -l`；USB 数据线设备优先（网络 serial 含 . 或 :）
fn parse_adb_devices(stdout: &str) -> Vec<AdbDevice> {
    let mut out = Vec::new();
    for line in stdout.lines().skip(1) {
        let mut it = line.split_whitespace();
        let (Some(serial), Some(state)) = (it.next(), it.next()) else {
            continue;
        };
        let rest: Vec<&str> = it.collect();
        let model = rest
            .iter()
            .find_map(|t| t.strip_prefix("model:"))
            .unwrap_or("")
            .to_string();
        let is_network = serial.contains(':') || serial.contains('.');
        out.push(AdbDevice {
            serial: serial.to_string(),
            state: state.to_string(),
            model,
            is_network,
        });
    }
    out
}

/// 当前在线（state == "device"）的 adb 设备；USB 优先，否则网络设备
pub fn adb_devices() -> anyhow::Result<Vec<AdbDevice>> {
    use std::os::windows::process::CommandExt;
    let adb = adb_exe()?;
    let out = Command::new(&adb)
        .arg("devices")
        .arg("-l")
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(parse_adb_devices(&String::from_utf8_lossy(&out.stdout)))
}

/// 当前在线（state == "device"）设备里自动挑一台：USB 优先，否则第一台网络设备
pub(crate) fn connected_serial() -> anyhow::Result<String> {
    let devs = adb_devices()?;
    if devs.is_empty() {
        anyhow::bail!("未找到 adb 设备（手机未通过 USB 连接，也没有已连接的无线设备）");
    }
    let online: Vec<&AdbDevice> = devs.iter().filter(|d| d.state == "device").collect();
    let pick = online
        .iter()
        .find(|d| !d.is_network)
        .or_else(|| online.first());
    match pick {
        Some(d) => Ok(d.serial.clone()),
        None => {
            let states = devs
                .iter()
                .map(|d| format!("{}={}", d.serial, d.state))
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!("检测到设备但均非 device 状态（{states}）——请检查手机授权弹窗或无线调试")
        }
    }
}

/// 校验并规范化无线调试地址，返回 host:port（未写端口默认 5555）
fn normalize_wifi_address(addr: &str) -> anyhow::Result<String> {
    let a = addr.trim();
    if a.is_empty() {
        anyhow::bail!("请输入手机地址，如 192.168.1.5:5555");
    }
    if a.len() > 255 {
        anyhow::bail!("地址过长");
    }
    if !a
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ':')
    {
        anyhow::bail!("地址只允许字母、数字、. - _ :");
    }
    let (host, port): (&str, &str) = match a.split(':').collect::<Vec<_>>().as_slice() {
        [h] => (h, "5555"),
        [h, p] if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
            let n: u32 = p.parse().map_err(|_| anyhow::anyhow!("端口无效"))?;
            if n == 0 || n > 65535 {
                anyhow::bail!("端口必须在 1-65535 之间");
            }
            (h, p)
        }
        _ => anyhow::bail!("地址格式应为 IP:端口，如 192.168.1.5:5555"),
    };
    if host.is_empty() {
        anyhow::bail!("地址主机名为空");
    }
    Ok(format!("{host}:{port}"))
}

/// adb connect <addr>；成功时返回 adb 输出原文
pub fn wifi_connect(address: &str) -> anyhow::Result<String> {
    use std::os::windows::process::CommandExt;
    let addr = normalize_wifi_address(address)?;
    let adb = adb_exe()?;
    let out = Command::new(&adb)
        .args(["connect", &addr])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    let ok = text.contains("connected to") || text.contains("already connected");
    if ok {
        log::info!("adb connect {addr} ok: {text}");
        Ok(text)
    } else {
        log::warn!("adb connect {addr} failed: {text}");
        anyhow::bail!("{text}")
    }
}

/// adb disconnect（不带参数 = 断开全部无线设备）
pub fn wifi_disconnect() -> anyhow::Result<String> {
    use std::os::windows::process::CommandExt;
    let adb = adb_exe()?;
    let out = Command::new(&adb)
        .arg("disconnect")
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    log::info!("adb disconnect: {text}");
    Ok(text)
}

/// 通过 USB 把设备切到 TCP/IP 模式（adb tcpip），拔线后可无线连接
pub fn wifi_tcpip() -> anyhow::Result<String> {
    use std::os::windows::process::CommandExt;
    let devs = adb_devices()?;
    let usb = devs
        .iter()
        .find(|d| d.state == "device" && !d.is_network)
        .map(|d| d.serial.clone());
    let Some(usb) = usb else {
        anyhow::bail!("需要一台通过 USB 连接且授权的手机，才能切到无线模式");
    };
    let adb = adb_exe()?;
    let out = Command::new(&adb)
        .args(["-s", &usb, "tcpip", "5555"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    let ok = text.contains("restarting") || out.status.success();
    if ok {
        log::info!("adb tcpip 5555 ({usb}): {text}");
        Ok(format!("{text}（现在可以拔掉 USB 数据线了）"))
    } else {
        anyhow::bail!("{text}")
    }
}

/// 回收已自行退出的 scrcpy 子进程；返回当前是否在运行
pub fn poll() -> bool {
    let mut guard = MIRROR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(info) = guard.as_mut() {
        match info.child.try_wait() {
            Ok(Some(status)) => {
                log::warn!("scrcpy exited on its own (serial={}, status={status})", info.serial);
                *guard = None;
            }
            Ok(None) => {}
            Err(e) => log::error!("scrcpy try_wait failed: {e}"),
        }
    }
    guard.is_some()
}

pub fn status() -> MirrorStatus {
    let running = poll();
    let guard = MIRROR.lock().unwrap_or_else(|e| e.into_inner());
    if running {
        match guard.as_ref() {
            Some(info) => MirrorStatus {
                running: true,
                serial: Some(info.serial.clone()),
                uptime_sec: Some(info.started_at.elapsed().as_secs()),
            },
            None => MirrorStatus {
                running: false,
                serial: None,
                uptime_sec: None,
            },
        }
    } else {
        MirrorStatus {
            running: false,
            serial: None,
            uptime_sec: None,
        }
    }
}

/// 主显示器工作区（扣除任务栏）：(left, top, right, bottom)
pub fn work_area() -> (i32, i32, i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut r as *mut RECT as *mut winapi::ctypes::c_void,
            0,
        )
    };
    if ok == 0 || r.right <= r.left {
        return (0, 0, 1920, 1040);
    }
    (r.left, r.top, r.right, r.bottom)
}

/// 侧边面板矩形：(x, y, w, h)，贴靠工作区右边缘
pub fn panel_rect() -> (i32, i32, i32, i32) {
    let (_, top, right, _) = work_area();
    (right - PANEL_WIDTH - 8, top + 24, PANEL_WIDTH, 462)
}

pub fn start() -> anyhow::Result<MirrorStatus> {
    start_with_serial(None)
}

/// 指定被投屏设备（serial 为 None 时 USB 优先自动选择）
pub fn start_with_serial(requested: Option<String>) -> anyhow::Result<MirrorStatus> {
    if poll() {
        return Ok(status());
    }
    let scrcpy = scrcpy_exe()?;
    let serial = match requested {
        Some(s) => {
            let s = s.trim().to_string();
            let online = adb_devices()?.iter().any(|d| d.serial == s && d.state == "device");
            if !online {
                anyhow::bail!("指定的设备 {s} 不在线（adb devices 里不是 device 状态）");
            }
            s
        }
        None => connected_serial()?,
    };
    let (_, top, right, bottom) = work_area();
    let panel_x = right - PANEL_WIDTH - 8;
    let window_x = (panel_x - 440).max(0);
    let window_y = top + 24;
    let height = (bottom - top - 48).clamp(480, 880);
    let title = format!("桌链投屏 · {serial}");

    let mut cmd = Command::new(&scrcpy);
    cmd.arg("-s")
        .arg(&serial)
        .arg("--no-audio")
        .arg("--stay-awake")
        .arg(format!("--window-title={title}"))
        .arg(format!("--window-x={window_x}"))
        .arg(format!("--window-y={window_y}"))
        .arg(format!("--window-height={height}"))
        // scrcpy 依赖同目录的 scrcpy-server / SDL.dll，显式设置工作目录
        .current_dir(scrcpy.parent().unwrap());
    let child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("scrcpy 启动失败: {e}"))?;
    log::info!("scrcpy started: serial={serial}, height={height}, x={window_x}, y={window_y}");
    *MIRROR.lock().unwrap_or_else(|e| e.into_inner()) = Some(MirrorInfo {
        child,
        serial,
        started_at: Instant::now(),
    });
    Ok(status())
}

/// 停止投屏；返回是否确有进程被回收
pub fn stop() -> bool {
    poll();
    let mut guard = MIRROR.lock().unwrap_or_else(|e| e.into_inner());
    match guard.take() {
        Some(mut info) => {
            let _ = info.child.kill();
            let _ = info.child.wait();
            log::info!("scrcpy stopped (serial={})", info.serial);
            true
        }
        None => false,
    }
}

/// 侧边面板按钮 → adb shell input keyevent（只作用于被投屏的手机）
pub fn send_keyevent(code: u32) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    poll();
    let serial = MIRROR
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|i| i.serial.clone());
    let Some(serial) = serial else {
        anyhow::bail!("投屏未运行，无法发送按键");
    };
    let adb = adb_exe()?;
    let key = code.to_string();
    let out = Command::new(&adb)
        .args(["-s", serial.as_str(), "shell", "input", "keyevent", &key])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !out.status.success() {
        anyhow::bail!("adb keyevent {code} 失败: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}
