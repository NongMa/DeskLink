// 通过 Windows API 发送键盘事件，实现对电脑的远程控制。

use winapi::um::winuser::{
    keybd_event, SendInput, INPUT, INPUT_MOUSE, INPUT_KEYBOARD, KEYBDINPUT, MOUSEINPUT,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    GUITHREADINFO,
    VK_MEDIA_NEXT_TRACK,
    VK_MEDIA_PLAY_PAUSE,
    VK_MEDIA_PREV_TRACK,
    VK_VOLUME_DOWN,
    VK_VOLUME_MUTE,
    VK_VOLUME_UP,
    VK_RETURN,
};
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::winuser::SW_SHOWNORMAL;
use winapi::um::winreg::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RegCloseKey, RegOpenKeyExW, RegQueryValueExW};
use winapi::um::winnt::{KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
use winapi::shared::minwindef::HKEY;

fn send_key(vk: i32) {
    unsafe {
        keybd_event(vk as u8, 0, 0, 0);
        keybd_event(vk as u8, 0, 2, 0); // 2 = KEYEVENTF_KEYUP
    }
}

// 组合键用到的虚拟键码（winapi 未全部导出，直接用数值）
const VK_TAB_KEY: u16 = 0x09;
const VK_SHIFT_KEY: u16 = 0x10;
const VK_CONTROL_KEY: u16 = 0x11;
const VK_MENU_KEY: u16 = 0x12; // Alt
const VK_LEFT_KEY: u16 = 0x25;
const VK_RIGHT_KEY: u16 = 0x27;
const VK_LWIN_KEY: u16 = 0x5B;
const VK_D_KEY: u16 = 0x44;
const VK_S_KEY: u16 = 0x53;
const VK_T_KEY: u16 = 0x54;
const VK_W_KEY: u16 = 0x57;
const VK_F4_KEY: u16 = 0x73;
const VK_F5_KEY: u16 = 0x74;
const VK_F11_KEY: u16 = 0x7A;

/// 组合键：先依次按下，再逆序抬起（如 Win+Shift+S、Alt+F4）
fn send_combo(keys: &[u16]) {
    for &vk in keys {
        unsafe { keybd_event(vk as u8, 0, 0, 0) };
    }
    for &vk in keys.iter().rev() {
        unsafe { keybd_event(vk as u8, 0, 2, 0) }; // 2 = KEYEVENTF_KEYUP
    }
}

// ---- 鼠标 ----

fn mouse_input(dx: i32, dy: i32, mouse_data: i32, flags: u32) -> INPUT {
    let mi = MOUSEINPUT {
        dx,
        dy,
        mouseData: mouse_data as u32,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };
    // winapi 的 INPUT_u 是不透明结构，zeroed 后通过 mi_mut() 写入
    let mut u: winapi::um::winuser::INPUT_u = unsafe { std::mem::zeroed() };
    *unsafe { u.mi_mut() } = mi;
    INPUT {
        type_: INPUT_MOUSE,
        u,
    }
}

fn send_mouse(i: &INPUT) {
    unsafe {
        SendInput(1, i as *const INPUT as winapi::um::winuser::LPINPUT, std::mem::size_of::<INPUT>() as i32);
    }
}

/// 相对移动鼠标（触摸板拖动用）
pub fn mouse_move(dx: i32, dy: i32) {
    let i = mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE);
    send_mouse(&i);
}

/// 鼠标按键：down=true 按下，false 抬起
pub fn mouse_button(button: &str, down: bool) -> anyhow::Result<()> {
    let flags = match (button, down) {
        ("left", true) => MOUSEEVENTF_LEFTDOWN,
        ("left", false) => MOUSEEVENTF_LEFTUP,
        ("right", true) => MOUSEEVENTF_RIGHTDOWN,
        ("right", false) => MOUSEEVENTF_RIGHTUP,
        ("middle", true) => MOUSEEVENTF_MIDDLEDOWN,
        ("middle", false) => MOUSEEVENTF_MIDDLEUP,
        _ => anyhow::bail!("unknown mouse button: {button}"),
    };
    let i = mouse_input(0, 0, 0, flags);
    send_mouse(&i);
    Ok(())
}

/// 单击（按下+抬起）
pub fn mouse_click(button: &str) -> anyhow::Result<()> {
    mouse_button(button, true)?;
    mouse_button(button, false)
}

/// 滚轮滚动，delta>0 向上
pub fn mouse_wheel(delta: i32) {
    let i = mouse_input(0, 0, delta, MOUSEEVENTF_WHEEL);
    send_mouse(&i);
}

/// 回车键（「确定」按钮：确认对话框/搜索/换行）
pub fn send_enter() {
    // keybd_event 兼容性最好，沿用现有实现
    send_key(VK_RETURN as i32);
}

fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 读取 HKLM/HKCU 下 App Paths\<exe> 的默认值（真实安装路径）。
/// 64 位视图为主，32 位视图兜底（32 位应用装在 WOW6432Node 里）。
fn reg_app_path_view(root: HKEY, subkey: &str, wow64: u32) -> Option<String> {
    let sub = wide_nul(subkey);
    let mut hkey: HKEY = std::ptr::null_mut();
    let rc = unsafe { RegOpenKeyExW(root, sub.as_ptr(), 0, KEY_READ | wow64, &mut hkey) };
    if rc != 0 {
        return None;
    }
    let mut buf = [0u16; 512];
    let mut bytes = (buf.len() * 2) as u32;
    let mut typ: u32 = 0;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut typ,
            buf.as_mut_ptr() as *mut u8,
            &mut bytes,
        )
    };
    unsafe { RegCloseKey(hkey) };
    if rc != 0 {
        return None;
    }
    // bytes 含结尾的 \0
    let chars = (bytes as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..chars]))
}

fn registry_app_path(exe: &str) -> Option<String> {
    let sub = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
    reg_app_path_view(HKEY_LOCAL_MACHINE, &sub, 0)
        .or_else(|| reg_app_path_view(HKEY_LOCAL_MACHINE, &sub, KEY_WOW64_64KEY))
        .or_else(|| reg_app_path_view(HKEY_LOCAL_MACHINE, &sub, KEY_WOW64_32KEY))
        .or_else(|| reg_app_path_view(HKEY_CURRENT_USER, &sub, 0))
}

fn open_app_args(exe: &str, args: Option<&str>) -> anyhow::Result<()> {
    let wide = wide_nul(exe);
    let wide_ptr = wide.as_ptr();
    let args_wide = args.map(wide_nul);
    let args_ptr = args_wide.as_ref().map(|v| v.as_ptr()).unwrap_or(std::ptr::null());
    let ret = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            wide_ptr,
            args_ptr,
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW 返回值 <= 32 表示失败（如 SE_ERR_NOASSOC 未找到关联程序）
    if (ret as isize) <= 32 {
        anyhow::bail!("ShellExecute({exe}) failed with code {}", ret as isize);
    }
    Ok(())
}

fn open_app(exe: &str) -> anyhow::Result<()> {
    open_app_args(exe, None)
}

/// 打开浏览器：优先 Edge（App Paths 注册的真实路径，带 about:blank 参数），
/// 再试常见安装位置，最后兜底用默认浏览器打开 https 链接（https 协议一定有关联）。
fn open_browser() -> anyhow::Result<()> {
    if let Some(path) = registry_app_path("msedge.exe") {
        if std::path::Path::new(&path).exists() {
            return open_app_args(&path, Some("about:blank"));
        }
    }
    for p in [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ] {
        if std::path::Path::new(p).exists() {
            return open_app_args(p, Some("about:blank"));
        }
    }
    open_app("https://www.bing.com")
}

/// 「打开<应用>」语音指令的中文别名 → 可执行文件名（小写键匹配）
/// 未命中别名时，把原名 + ".exe" 当作 exe 名走 App Paths / PATH 查找
fn known_app(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        // 社交/通讯
        "微信" | "wechat" | "weixin" => &["WeChat.exe", "weixin.exe"],
        "企业微信" | "wxwork" => &["WXWork.exe"],
        "qq" | "腾讯qq" | "tim" => &["TIM.exe", "QQ.exe"],
        "钉钉" | "dingtalk" => &["DingTalk.exe"],
        "飞书" | "feishu" | "lark" => &["Feishu.exe", "Lark.exe"],
        "腾讯会议" | "wemeet" => &["WeMeetApp.exe", "WeMeet.exe"],
        "zoom" => &["Zoom.exe"],
        "teams" | "microsoft teams" | "microsoftteams" => &["MSTeams.exe", "Teams.exe", "ms-teams.exe"],
        // 音乐/影音
        "网易云音乐" | "网易云" | "cloudmusic" => &["cloudmusic.exe"],
        "酷狗音乐" | "酷狗" | "kugou" => &["KGMusic.exe", "KuGou.exe"],
        "酷我音乐" | "酷我" | "kwmusic" => &["KwMusic.exe", "kwmusic.exe"],
        "qq音乐" | "qqmusic" => &["QQMusic.exe"],
        "腾讯视频" | "qqlive" => &["QQLive.exe"],
        "爱奇艺" => &["QyClient.exe"],
        "哔哩哔哩" | "bilibili" | "b站" => &["哔哩哔哩.exe", "bilibili.exe"],
        "抖音" | "douyin" | "tiktok" => &["douyin.exe"],
        // 办公/开发
        "wps" | "wps文字" | "wpsword" | "wps演示" | "wpp" => &["wps.exe", "wpp.exe"],
        "word" => &["WINWORD.EXE", "wps.exe"],
        "excel" => &["EXCEL.EXE", "wps.exe"],
        "powerpoint" | "ppt" => &["POWERPNT.EXE", "wps.exe"],
        "outlook" => &["OUTLOOK.EXE"],
        "vscode" | "vs code" | "code" | "visualstudiocode" => &["Code.exe"],
        // 工具/游戏
        "steam" => &["steam.exe"],
        "迅雷" | "thunder" => &["Thunder.exe"],
        "百度网盘" | "netdisk" => &["BaiduNetdisk.exe", "xpan.exe"],
        "剪映" => &["JianyingPro.exe"],
        // 系统工具
        "任务管理器" => &["taskmgr.exe"],
        "资源管理器" | "文件管理器" => &["explorer.exe"],
        "控制面板" => &["control.exe"],
        "画图" => &["mspaint.exe"],
        _ => return None,
    })
}

/// 尝试启动一个 exe：先查 App Paths 注册表拿真实安装路径，
/// 再 ShellExecute 直接解析，最后 cmd /c start 兜底（start 同样走 ShellExecuteEx）
fn try_open_exe(exe: &str) -> bool {
    if let Some(path) = registry_app_path(exe) {
        if std::path::Path::new(&path).exists() && open_app(&path).is_ok() {
            return true;
        }
    }
    if open_app(exe).is_ok() {
        return true;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .arg("/c")
        .arg("start")
        .arg("") // start 的窗口标题占位参数
        .arg(format!("\"{exe}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
}

/// 「打开<应用>」通用入口：先查本机动态目录（快捷方式/UWP，最准），
/// 未命中再退到内置别名表与 exe 名查找
fn open_named_app(name: &str) -> anyhow::Result<()> {
    let key = name.trim().trim_matches(['。', '.', '，', ',', '？', '?', '！', '!']).to_lowercase();
    if key.is_empty() {
        anyhow::bail!("未听清要打开什么应用");
    }
    // 1) 动态目录：根据这台电脑实际装了什么查（App 别名表已不再局限写死列表）
    if let Some(target) = crate::appcat::lookup(&key) {
        log::info!("open_app: '{}' -> {}", key, target);
        let launched = if target.starts_with("shell:AppsFolder\\") {
            // UWP 应用：用 explorer.exe 解析 shell:AppsFolder\AppID 启动
            open_app_args("explorer.exe", Some(&target)).map(|_| true)
        } else {
            open_app(&target).map(|_| true)
        };
        if launched.is_ok() {
            return Ok(());
        }
        log::warn!("open_app: 目录匹配到 {} 但启动失败，继续兜底", target);
    }
    // 2) 内置别名表兜底
    let candidates: Vec<String> = match known_app(&key) {
        Some(list) => list.iter().map(|s| s.to_string()).collect(),
        None => {
            let mut v = Vec::with_capacity(2);
            if key.ends_with(".exe") {
                v.push(key.clone());
            } else {
                v.push(format!("{key}.exe"));
            }
            v.push(key.clone());
            v
        }
    };
    for exe in &candidates {
        if try_open_exe(exe) {
            return Ok(());
        }
    }
    anyhow::bail!("这台电脑上没找到应用「{name}」（已查快捷方式目录、App Paths 注册表、PATH）")
}

/// 向当前拥有键盘焦点的控件注入文本（Unicode 键盘事件，逐 UTF-16 码元发送。
/// 中文等非 ASCII 字符不走 VK 映射，直接走 KEYEVENTF_UNICODE）
pub fn type_text(text: &str) {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.encode_utf16().count() * 2);
    for unit in text.encode_utf16() {
        for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
            let ki = KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            };
            let mut u: winapi::um::winuser::INPUT_u = unsafe { std::mem::zeroed() };
            *unsafe { u.ki_mut() } = ki;
            inputs.push(INPUT {
                type_: INPUT_KEYBOARD,
                u,
            });
        }
    }
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr() as winapi::um::winuser::LPINPUT,
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// 兜底焦点检测：按焦点控件的窗口类名判断（Edit/RichEdit/Scintilla 等）。
/// UIA 不可用时启用。浏览器内输入框无法用类名区分，此处统一漏报（返回 false）。
fn foreground_input_focused_fallback() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }
        let tid = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        if tid == 0 {
            return false;
        }
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(tid, &mut info) == 0 {
            return false;
        }
        if info.hwndFocus.is_null() {
            return false;
        }
        let mut buf = [0u16; 128];
        let n = GetClassNameW(info.hwndFocus, buf.as_mut_ptr(), buf.len() as i32);
        if n <= 0 {
            return false;
        }
        let class = String::from_utf16_lossy(&buf[..n as usize]).to_lowercase();
        class.starts_with("edit")
            || class.starts_with("richedit")
            || class.starts_with("riched")
            || class.starts_with("windowsforms10.edit")
            || class.starts_with("scintilla")
            || class.starts_with("tedit")
            || class.starts_with("delphi_edit")
    }
}

/// UIA 焦点检测：焦点元素为 Edit / Document / ComboBox 即视为正在期待文本输入
/// （覆盖 Notepad、浏览器输入框、VS Code 文档编辑区、WinForms/WPF/WinUI TextBox 等）
fn uia_focused_element_editable() -> Option<bool> {
    use uiautomation::types::ControlType;
    let automation = uiautomation::UIAutomation::new()
        .or_else(|_| uiautomation::UIAutomation::new_direct())
        .ok()?;
    let element = automation.get_focused_element().ok()?;
    let control_type = element.get_control_type().ok()?;
    Some(matches!(
        control_type,
        ControlType::Edit | ControlType::Document | ControlType::ComboBox
    ))
}

/// 是否处于"文本输入"焦点状态：优先 UIA，失败时回退到类名启发式
pub fn foreground_input_focused() -> bool {
    uia_focused_element_editable().unwrap_or_else(foreground_input_focused_fallback)
}

pub fn exec_action(action: &str) -> anyhow::Result<()> {
    match action {
        "volume_up" => send_key(VK_VOLUME_UP as i32),
        "volume_down" => send_key(VK_VOLUME_DOWN as i32),
        "volume_mute" => send_key(VK_VOLUME_MUTE as i32),
        "play_pause" | "media_play_pause" => send_key(VK_MEDIA_PLAY_PAUSE as i32),
        "next_track" => send_key(VK_MEDIA_NEXT_TRACK as i32),
        "prev_track" => send_key(VK_MEDIA_PREV_TRACK as i32),

        // 打开 Edge（无 Edge 则打开默认浏览器）
        "open_chrome" | "open_browser" => { open_browser()?; }
        "open_notepad" => { open_app("notepad.exe")?; }
        "open_calc" => { open_app("calc.exe")?; }

        "enter" | "ok" => send_enter(),
        "left_click" => { mouse_click("left")?; }
        "right_click" => { mouse_click("right")?; }
        "wheel_up" => mouse_wheel(120),
        "wheel_down" => mouse_wheel(-120),

        // 系统级操作
        "lock_workstation" => {
            if unsafe { winapi::um::winuser::LockWorkStation() } == 0 {
                anyhow::bail!("LockWorkStation failed: {}", std::io::Error::last_os_error());
            }
        }
        "screenshot" => send_combo(&[VK_LWIN_KEY, VK_SHIFT_KEY, VK_S_KEY]),
        "show_desktop" => send_combo(&[VK_LWIN_KEY, VK_D_KEY]),
        "task_view" => send_combo(&[VK_LWIN_KEY, VK_TAB_KEY]),
        "alt_tab" | "switch_window" => send_combo(&[VK_MENU_KEY, VK_TAB_KEY]),
        "close_window" => send_combo(&[VK_MENU_KEY, VK_F4_KEY]),
        "fullscreen" => send_combo(&[VK_F11_KEY]),
        "go_back" => send_combo(&[VK_MENU_KEY, VK_LEFT_KEY]),
        "go_forward" => send_combo(&[VK_MENU_KEY, VK_RIGHT_KEY]),
        "refresh" => send_combo(&[VK_F5_KEY]),
        "tab_new" => send_combo(&[VK_CONTROL_KEY, VK_T_KEY]),
        "tab_close" => send_combo(&[VK_CONTROL_KEY, VK_W_KEY]),
        "tab_next" => send_combo(&[VK_CONTROL_KEY, VK_TAB_KEY]),
        // 投屏（scrcpy）：start/stop 幂等；这里不区分成功失败细节，失败时错误会进日志
        "mirror_start" => {
            crate::mirror::start().map(|_| ())?;
        }
        "mirror_stop" => {
            crate::mirror::stop();
        }

        "open_taskmgr" => { open_app("taskmgr.exe")?; }
        "open_explorer" => { open_app("explorer.exe")?; }
        "open_control" => { open_app("control.exe")?; }
        // ms-settings: 是 Inbox URI，ShellExecute 能直接解析
        "open_settings" => { open_app("ms-settings:")?; }
        "open_paint" => { open_app("mspaint.exe")?; }

        "hello" => println!("Hello from VRC server, phone!"),

        unknown => {
            if let Some(name) = unknown.strip_prefix("open_app:") {
                open_named_app(name)?;
            } else {
                return Err(anyhow::anyhow!("unknown action: {unknown}"));
            }
        }
    }
    Ok(())
}
