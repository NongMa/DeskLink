//! Windows 托盘图标（win32 裸 API + winapi 0.3）
//! 主线程跑消息循环；tokio 服务器在后台线程。
//! 右键菜单：打开管理页 / 投屏子菜单（自动投屏 · 每台设备 · 结束投屏）/ 复制配对链接 / 重新生成令牌 / 令牌认证开关 / 开机自启 / 退出

use std::sync::{Arc, Mutex};

use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::shellapi::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP,
    NIM_ADD, NIM_DELETE, NIM_MODIFY,
};
use winapi::um::winbase::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT};
use winapi::shared::minwindef::{HGLOBAL, HINSTANCE};
use winapi::shared::windef::{HICON, HWND, HMENU, POINT};
use winapi::um::winuser::*;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};

use crate::state::AppState;

const CLASS_NAME: &str = "VrcTrayWindow";
const WM_TRAYICON: UINT = WM_USER + 1;

const ID_OPEN: u16 = 101;
const ID_COPY: u16 = 102;
const ID_REGEN: u16 = 103;
const ID_AUTH: u16 = 104;
const ID_AUTOSTART: u16 = 105;
const ID_QUIT: u16 = 106;
// 投屏子菜单（1100 起）：1100 自动投屏 / 1101 结束投屏 / 1110..1125 选择设备
const ID_MIRROR_AUTO: u16 = 1100;
const ID_MIRROR_STOP: u16 = 1101;
const ID_MIRROR_DEV_BASE: u16 = 1110;
const MIRROR_DEV_MAX: usize = 16;

/// 托盘菜单展开设备列表时取一份快照，点击时按索引取回（菜单销毁后仍然可靠）
static MIRROR_MENU_DEVICES: Mutex<Vec<crate::mirror::AdbDevice>> = Mutex::new(Vec::new());

/// 供静态 WndProc 访问的共享状态
static APP: Mutex<Option<Arc<AppState>>> = Mutex::new(None);

fn wide_z(s: &str) -> Vec<u16> {
    let mut v = s.encode_utf16().collect::<Vec<_>>();
    v.push(0);
    v
}

/// 从内嵌的 .ico 文件中挑出最合适的一帧（RT_ICON 资源位），
/// CreateIconFromResourceEx 不接受整个 .ico 容器，必须先剥离 ICONDIR
fn pick_ico_frame(bytes: &[u8], want: u16) -> Option<&[u8]> {
    if bytes.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut frames: Vec<(u16, usize, usize)> = Vec::with_capacity(count); // (width, byte_size, file_offset)
    for i in 0..count {
        let e = 6 + i * 16;
        if e + 16 > bytes.len() {
            break;
        }
        let raw_w = bytes[e] as u16; // 0 表示 256
        let width = if raw_w == 0 { 256 } else { raw_w };
        let img_size = u32::from_le_bytes([bytes[e + 8], bytes[e + 9], bytes[e + 10], bytes[e + 11]]) as usize;
        let offset = u32::from_le_bytes([bytes[e + 12], bytes[e + 13], bytes[e + 14], bytes[e + 15]]) as usize;
        if img_size == 0 {
            continue;
        }
        let Some(end) = offset.checked_add(img_size) else { continue };
        if end > bytes.len() {
            continue;
        }
        frames.push((width, img_size, offset));
    }
    let frame = frames.iter().filter(|f| f.0 >= want).min_by_key(|f| f.0)
        .or_else(|| frames.iter().max_by_key(|f| f.0))?;
    Some(&bytes[frame.2..frame.2 + frame.1])
}

fn load_icon() -> Option<HICON> {
    let bytes = include_bytes!("../assets/vrc.ico");
    let frame = match pick_ico_frame(bytes, 32) {
        Some(f) => f,
        None => {
            log::error!("tray icon: vrc.ico has no usable frame; using system default");
            return None;
        }
    };
    let hicon = unsafe {
        CreateIconFromResourceEx(
            frame.as_ptr() as *mut u8,
            frame.len() as u32,
            1, /* TRUE：图标（非光标） */
            0x00030000,
            32,
            32,
            0,
        )
    };
    if hicon.is_null() {
        log::error!("tray icon: CreateIconFromResourceEx failed; using system default");
        None
    } else {
        Some(hicon)
    }
}

fn copy_to_clipboard(text: &str) {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return;
        }
        EmptyClipboard();
        let data = wide_z(text);
        let h: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, data.len() * 2);
        if !h.is_null() {
            let p = GlobalLock(h);
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), p as *mut u16, data.len());
                GlobalUnlock(h);
                SetClipboardData(13 /* CF_UNICODETEXT */, h);
            }
        }
        CloseClipboard();
    }
}

/// 定位 Tauri 管理端 vrc-admin.exe：
/// 1) 环境变量 VRC_ADMIN_EXE 显式指定
/// 2) 与服务同目录（发布形态：两个 exe 放一起）
/// 3) 开发形态：从本 exe 逐级向上查找 tauri-admin/src-tauri/target/release/vrc-admin.exe（Tauri v2 产物位于 src-tauri/target）
fn admin_exe() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("VRC_ADMIN_EXE") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let cur = std::env::current_exe().ok()?;
    let dir = cur.parent()?.to_path_buf();
    let sibling = dir.join("vrc-admin.exe");
    if sibling.exists() {
        return Some(sibling);
    }
    let mut d = dir;
    loop {
        for rel in [
            &["tauri-admin", "src-tauri", "target", "release", "vrc-admin.exe"][..],
            &["tauri-admin", "target", "release", "vrc-admin.exe"][..],
        ] {
            let mut cand = d.clone();
            for part in rel {
                cand.push(part);
            }
            if cand.exists() {
                return Some(cand);
            }
        }
        match d.parent() {
            Some(p) => d = p.to_path_buf(),
            None => return None,
        }
    }
}

/// 打开管理端：已有窗口则前置，否则启动 vrc-admin.exe
fn launch_admin(hwnd: HWND) {
    let title = wide_z("桌链 DeskLink 管理");
    unsafe {
        let existing = FindWindowW(std::ptr::null_mut(), title.as_ptr());
        if !existing.is_null() {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
            log::info!("admin window focused (already running)");
            return;
        }
    }
    match admin_exe() {
        Some(exe) => match std::process::Command::new(&exe).spawn() {
            Ok(_) => log::info!("admin app launched: {}", exe.display()),
            Err(e) => {
                log::error!("failed to launch admin app {}: {e}", exe.display());
                show_balloon(hwnd, "桌链", "管理端启动失败，请查看日志");
            }
        },
        None => {
            log::error!("vrc-admin.exe not found (set VRC_ADMIN_EXE or build tauri-admin)");
            show_balloon(hwnd, "桌链", "未找到管理端 vrc-admin.exe");
        }
    }
}

/// 托盘启动投屏：已在投屏且目标相同则提示；目标不同则先结束当前投屏再切换
fn start_mirror(hwnd: HWND, serial: Option<String>) {
    let cur = crate::mirror::status();
    if cur.running {
        let same = match (&serial, &cur.serial) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, _) => true, // 「自动」视为当前正在投屏，不重复启动
            _ => false,
        };
        if same {
            show_balloon(hwnd, "桌链", "投屏已在进行中");
            return;
        }
        log::info!("mirror switching device: {} -> {}", cur.serial.unwrap_or_default(), serial.as_deref().unwrap_or("auto"));
        crate::mirror::stop();
        // 给 scrcpy 一点退出时间，避免新旧进程抢同一个 serial 的投屏
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    match crate::mirror::start_with_serial(serial.clone()) {
        Ok(st) => {
            let target = st.serial.unwrap_or_default();
            log::info!("mirror started via tray: {target}");
            show_balloon(hwnd, "桌链", &format!("投屏已开始 · {target}（关闭投屏面板即结束）"));
        }
        Err(e) => {
            log::error!("mirror start failed: {e:#}");
            show_balloon(hwnd, "桌链", &format!("投屏启动失败：{e:#}"));
        }
    }
}

fn show_balloon(hwnd: HWND, title: &str, text: &str) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_INFO;
        let mut info = [0u16; 256];
        for (i, c) in text.encode_utf16().take(255).enumerate() {
            info[i] = c;
        }
        nid.szInfo = info;
        let mut ti = [0u16; 64];
        for (i, c) in title.encode_utf16().take(63).enumerate() {
            ti[i] = c;
        }
        nid.szInfoTitle = ti;
        nid.dwInfoFlags = 1; // NIIF_INFO
        Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
}

fn menu_item(menu: HMENU, flags: UINT, id: u16, text: &str) {
    let t = wide_z(text);
    unsafe {
        AppendMenuW(menu, flags, id as usize, t.as_ptr());
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            match lparam as u32 {
                WM_RBUTTONUP => show_menu(hwnd),
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    launch_admin(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            if wparam as UINT == crate::panel::SYNC_TIMER_ID {
                crate::panel::sync();
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as u16;
            match id {
                ID_OPEN => {
                    launch_admin(hwnd);
                }
                ID_COPY => {
                    if let Some(st) = APP.lock().unwrap().as_ref().cloned() {
                        copy_to_clipboard(&st.pair_link());
                        log::info!("pairing link copied to clipboard");
                        show_balloon(hwnd, "桌链", "配对链接已复制到剪贴板");
                    }
                }
                ID_REGEN => {
                    if let Some(st) = APP.lock().unwrap().as_ref().cloned() {
                        st.regenerate_token();
                        log::warn!("pairing token regenerated from tray");
                        show_balloon(hwnd, "桌链", "配对令牌已重新生成，手机需要重新配对");
                    }
                }
                ID_AUTH => {
                    if let Some(st) = APP.lock().unwrap().as_ref().cloned() {
                        let now = st.auth_required();
                        st.set_auth_required(!now);
                        st.persist();
                        log::info!("token auth {} via tray", if !now { "enabled" } else { "disabled" });
                    }
                }
                ID_AUTOSTART => {
                    let now = crate::autostart::is_enabled();
                    match crate::autostart::set_enabled(!now) {
                        Ok(()) => log::info!("autostart set to {} via tray", !now),
                        Err(e) => log::error!("failed to toggle autostart: {e:#}"),
                    }
                }
                ID_MIRROR_AUTO => {
                    start_mirror(hwnd, None);
                }
                ID_MIRROR_STOP => {
                    let was_running = crate::mirror::status().running;
                    crate::mirror::stop();
                    log::info!("mirror stopped via tray (was_running={was_running})");
                    show_balloon(hwnd, "桌链", "投屏已结束");
                }
                id if (ID_MIRROR_DEV_BASE..ID_MIRROR_DEV_BASE + MIRROR_DEV_MAX as u16).contains(&id) => {
                    let idx = (id - ID_MIRROR_DEV_BASE) as usize;
                    let devices = MIRROR_MENU_DEVICES.lock().unwrap().clone();
                    match devices.get(idx) {
                        Some(d) => start_mirror(hwnd, Some(d.serial.clone())),
                        None => log::warn!("mirror menu device index {idx} out of range, ignored"),
                    }
                }
                ID_QUIT => {
                    crate::mirror::stop();
                    log::info!("tray quit requested");
                    std::process::exit(0);
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Some(st) = APP.lock().unwrap().as_ref().cloned() else {
        return;
    };
    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    // 状态行（置灰不可点）
    let clients = st.clients().len();
    let whisper = match crate::state::whisper_state() {
        1 => "Whisper 就绪",
        2 => "Whisper 加载失败",
        _ => "Whisper 加载中…",
    };
    menu_item(
        menu,
        MF_STRING | MF_DISABLED | MF_GRAYED,
        0,
        &format!("{}:{} · 已连接 {} 台 · {}", st.lan_ip, st.port, clients, whisper),
    );
    menu_item(menu, MF_SEPARATOR, 0, "");
    menu_item(menu, MF_STRING, ID_OPEN, "打开管理页(&O)");
    menu_item(menu, MF_STRING, ID_COPY, "复制配对链接(&C)");
    // 投屏子菜单：自动（优先 USB）/ 每台在线设备 / 结束投屏
    let mirror_status = crate::mirror::status();
    let mirror_running = mirror_status.running;
    let mirror_sub = CreatePopupMenu();
    if !mirror_sub.is_null() {
        if mirror_running {
            menu_item(
                mirror_sub,
                MF_STRING | MF_DISABLED | MF_GRAYED,
                0,
                &format!("投屏中 · {}", mirror_status.serial.clone().unwrap_or_default()),
            );
            menu_item(mirror_sub, MF_SEPARATOR, 0, "");
        }
        menu_item(mirror_sub, MF_STRING, ID_MIRROR_AUTO, "自动投屏（优先 USB）(&A)");
        let devs = crate::mirror::adb_devices().unwrap_or_default();
        let online: Vec<crate::mirror::AdbDevice> = devs
            .into_iter()
            .filter(|d| d.state == "device")
            .take(MIRROR_DEV_MAX)
            .collect();
        *MIRROR_MENU_DEVICES.lock().unwrap() = online.clone();
        if online.is_empty() {
            menu_item(mirror_sub, MF_STRING | MF_DISABLED | MF_GRAYED, 0, "未检测到在线设备");
        } else {
            for (i, d) in online.iter().enumerate() {
                let tag = if d.is_network { "WiFi" } else { "USB" };
                let model = if d.model.is_empty() { String::new() } else { format!(" · {}", d.model) };
                menu_item(mirror_sub, MF_STRING, ID_MIRROR_DEV_BASE + i as u16, &format!("{tag} · {}{}", d.serial, model));
            }
        }
        if mirror_running {
            menu_item(mirror_sub, MF_SEPARATOR, 0, "");
            menu_item(mirror_sub, MF_STRING, ID_MIRROR_STOP, "结束投屏(&E)");
        }
        let mut sub_flags = MF_POPUP;
        if mirror_running {
            sub_flags |= MF_CHECKED;
        }
        let label = if mirror_running { "投屏中(&M)" } else { "投屏(&M)" };
        let label_w = wide_z(label);
        AppendMenuW(menu, sub_flags, mirror_sub as usize, label_w.as_ptr()); // 父菜单接管子菜单
    }
    menu_item(menu, MF_STRING, ID_REGEN, "重新生成配对令牌(&R)");
    menu_item(menu, MF_SEPARATOR, 0, "");
    let auth_flags = if st.auth_required() {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    menu_item(menu, auth_flags, ID_AUTH, "强制配对令牌认证(&T)");
    let auto_flags = if crate::autostart::is_enabled() {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    menu_item(menu, auto_flags, ID_AUTOSTART, "开机自启(&S)");
    menu_item(menu, MF_SEPARATOR, 0, "");
    menu_item(menu, MF_STRING, ID_QUIT, "退出(&Q)");

    // 让菜单失去焦点时能自动收起
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, std::ptr::null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
}

/// 在主线程创建托盘图标并阻塞进入消息循环（直到进程退出）
pub fn run(state: Arc<AppState>) -> ! {
    *APP.lock().unwrap() = Some(state.clone());

    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null_mut());
        let class = wide_z(CLASS_NAME);

        let wc = wnd_class_w(wnd_proc, class.as_ptr(), hinst);
        if RegisterClassW(&wc) == 0 {
            // 类注册失败也继续尝试（可能重复）
            log::warn!("RegisterClassW failed or already registered");
        }

        let title = wide_z("VoiceRemoteControl");
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            panic!("failed to create tray window");
        }

        let icon = load_icon();
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        if let Some(h) = icon {
            nid.hIcon = h;
        } else {
            nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }
        let mut tip = [0u16; 128];
        for (i, c) in "桌链 DeskLink\n手机与 Windows 协同 · 右键查看菜单"
            .encode_utf16()
            .take(127)
            .enumerate()
        {
            tip[i] = c;
        }
        nid.szTip = tip;
        if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
            log::error!("Shell_NotifyIconW(NIM_ADD) failed");
        } else {
            log::info!("tray icon added");
        }

        // 注册投屏侧边面板窗口类 + 500ms 定时器（驱动面板随投屏状态显隐）
        crate::panel::init();
        if SetTimer(hwnd, crate::panel::SYNC_TIMER_ID as usize, 500, None) == 0 {
            log::error!("SetTimer for mirror panel sync failed");
        }

        // 消息循环
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // 清理图标
        Shell_NotifyIconW(NIM_DELETE, &mut nid);
        DestroyWindow(hwnd);
    }
    std::process::exit(0);
}

fn wnd_class_w(
    wnd_proc: unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
    class_name: *const u16,
    hinst: HINSTANCE,
) -> WNDCLASSW {
    let mut wc: WNDCLASSW = unsafe { std::mem::zeroed() };
    wc.style = 0;
    wc.lpfnWndProc = Some(wnd_proc);
    wc.hInstance = hinst;
    wc.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    wc.lpszClassName = class_name;
    wc
}
