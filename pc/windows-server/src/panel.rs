//! 投屏侧边控制面板（win32，与托盘共用主线程消息循环）
//! 圆角深色面板：标题栏可拖拽 + 关闭按钮，8 个 owner-drawn 按键（返回 / 主页 / 最近 /
//! 音量 / 电源 / 通知 / 截屏），按键走 adb keyevent，只作用于被投屏的手机。
//! 关闭面板即结束投屏；面板显隐由托盘 WM_TIMER 驱动（crate::panel::sync）。

use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HFONT, HDC, HWND, RECT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::{
    CreateFontW, CreatePen, CreateSolidBrush, CreateRoundRectRgn, DeleteObject, SelectObject, RoundRect, Ellipse,
    Polygon, MoveToEx, LineTo, SetBkMode, SetTextColor, GetStockObject, Rectangle,
    TRANSPARENT, PS_SOLID, NULL_BRUSH, NULL_PEN,
};
use winapi::um::winuser::*;

const CLASS_NAME: &str = "VrcMirrorPanel";
const SYNC_TIMER: UINT = 2;

const PANEL_W: i32 = 236;
const PANEL_H: i32 = 462;

const BTN_X: i32 = 14;
const BTN_W: i32 = 208;
const BTN_H: i32 = 38;
const BTN_GAP: i32 = 6;
const BTN_Y0: i32 = 66;

const C_BG: u32 = 0x00_1C1B1B;      // 深色底（BGR 0x001C1B1B ≈ RGB 27,27,28）
const C_CARD: u32 = 0x00_2A2929;   // 按钮卡底
const C_CARD_HOV: u32 = 0x00_3A3737;
const C_CARD_PRESS: u32 = 0x00_4A463F;
const C_STROKE: u32 = 0x00_3D3D3D;
const C_TEXT: u32 = 0x00_F2F0ED;
const C_DIM: u32 = 0x00_A8A49E;
const C_ACCENT: u32 = 0x00_D8A23F; // 品牌蓝（BGR of #3FA2D8 ≈ 0xD8A23F）
const C_CLOSE: u32 = 0x00_5F5CF0;  // 红色关闭（BGR ≈ #F05C5F）

const ID_KEY_BACK: u16 = 201;
const ID_KEY_HOME: u16 = 202;
const ID_KEY_RECENTS: u16 = 203;
const ID_KEY_VOL_UP: u16 = 204;
const ID_KEY_VOL_DOWN: u16 = 205;
const ID_KEY_POWER: u16 = 206;
const ID_KEY_NOTIF: u16 = 207;
const ID_KEY_SCREENSHOT: u16 = 208;
const ID_CLOSE: u16 = 209;

const KEYCODES: &[(u16, u32)] = &[
    (ID_KEY_BACK, 4), (ID_KEY_HOME, 3), (ID_KEY_RECENTS, 187), (ID_KEY_VOL_UP, 24),
    (ID_KEY_VOL_DOWN, 25), (ID_KEY_POWER, 26), (ID_KEY_NOTIF, 83), (ID_KEY_SCREENSHOT, 313),
];
const BUTTONS: &[(u16, &str)] = &[
    (ID_KEY_BACK, "返回"), (ID_KEY_HOME, "主页"), (ID_KEY_RECENTS, "最近任务"),
    (ID_KEY_VOL_UP, "音量 +"), (ID_KEY_VOL_DOWN, "音量 −"),
    (ID_KEY_POWER, "电源"), (ID_KEY_NOTIF, "通知栏"), (ID_KEY_SCREENSHOT, "截屏"),
];

static PANEL_HWND: AtomicIsize = AtomicIsize::new(0);
static FONT_LABEL: AtomicIsize = AtomicIsize::new(0);
static FONT_TITLE: AtomicIsize = AtomicIsize::new(0);
static HOVER_BTN: Mutex<u16> = Mutex::new(0);
static PRESS_BTN: Mutex<u16> = Mutex::new(0);
static SER_TEXT: Mutex<String> = Mutex::new(String::new());

fn wide_z(s: &str) -> Vec<u16> {
    let mut v = s.encode_utf16().collect::<Vec<_>>();
    v.push(0);
    v
}
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}
fn c(v: u32) -> u32 { rgb((v & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v >> 16) as u8) }

fn hinst() -> HINSTANCE { unsafe { GetModuleHandleW(ptr::null()) } }
fn panel_hwnd() -> HWND { PANEL_HWND.load(Ordering::Relaxed) as HWND }

pub fn init() {
    let class = wide_z(CLASS_NAME);
    let mut wc: WNDCLASSW = unsafe { std::mem::zeroed() };
    wc.lpfnWndProc = Some(panel_wnd_proc);
    wc.hInstance = hinst();
    wc.hCursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    wc.lpszClassName = class.as_ptr();
    unsafe {
        RegisterClassW(&wc);
    }
    let fl = unsafe {
        CreateFontW(-14, 0, 0, 0, 500, 0, 0, 0, 129, 0, 0, 4, 0, wide_z("Microsoft YaHei UI").as_ptr())
    };
    let ft = unsafe {
        CreateFontW(-17, 0, 0, 0, 700, 0, 0, 0, 129, 0, 0, 4, 0, wide_z("Microsoft YaHei UI").as_ptr())
    };
    FONT_LABEL.store(fl as isize, Ordering::Relaxed);
    FONT_TITLE.store(ft as isize, Ordering::Relaxed);
}

pub fn sync() {
    let preview = std::env::var_os("VRC_PANEL_PREVIEW").is_some();
    let running = crate::mirror::poll();
    let hwnd = panel_hwnd();
    if (running || preview) && hwnd.is_null() {
        create_panel();
    } else if !running && !preview && !hwnd.is_null() {
        unsafe { DestroyWindow(hwnd); }
        PANEL_HWND.store(0, Ordering::Relaxed);
    }
}

fn create_panel() {
    let (x, y, w, h) = crate::mirror::panel_rect();
    let _ = (w, h);
    let class = wide_z(CLASS_NAME);
    let title = wide_z("投屏控制");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            x, y, PANEL_W, PANEL_H,
            ptr::null_mut(), ptr::null_mut(), hinst(), ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        log::error!("mirror panel: CreateWindowExW failed");
        return;
    }
    match crate::mirror::status().serial {
        Some(s) => *SER_TEXT.lock().unwrap() = s,
        None => *SER_TEXT.lock().unwrap() = "未连接".to_string(),
    }
    // 圆角窗口形状
    let rgn = unsafe { CreateRoundRectRgn(0, 0, PANEL_W + 1, PANEL_H + 1, 28, 28) };
    if !rgn.is_null() {
        unsafe { SetWindowRgn(hwnd, rgn, 1); } // 窗口接管 rgn 所有权
    }
    PANEL_HWND.store(hwnd as isize, Ordering::Relaxed);
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, PANEL_W, PANEL_H, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
    log::info!("mirror panel created");
}

fn btn_rect(i: usize) -> (i32, i32, i32, i32) {
    let top = BTN_Y0 + i as i32 * (BTN_H + BTN_GAP);
    (BTN_X, top, BTN_X + BTN_W, top + BTN_H)
}
fn btn_index_at(x: i32, y: i32) -> Option<usize> {
    BUTTONS.iter().enumerate().find_map(|(i, _)| {
        let (l, t, r, b) = btn_rect(i);
        if x >= l && x <= r && y >= t && y <= b { Some(i) } else { None }
    })
}
fn close_rect() -> (i32, i32, i32, i32) {
    let cx = PANEL_W - 32;
    let cy = 26;
    (cx - 12, cy - 12, cx + 12, cy + 12)
}
fn in_close(x: i32, y: i32) -> bool {
    let (l, t, r, b) = close_rect();
    x >= l && x <= r && y >= t && y <= b
}

fn keyevent_for(id: u16) -> Option<u32> {
    KEYCODES.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

unsafe fn draw_text(hdc: HDC, font: isize, rect: (i32, i32, i32, i32), s: &str, col: u32, align: u32) {
    let (l, t, r, b) = rect;
    let mut rc = RECT { left: l, top: t, right: r, bottom: b };
    let hfont = font as HFONT;
    let old = SelectObject(hdc, hfont as _);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, c(col));
    let mut buf: Vec<u16> = s.encode_utf16().collect();
    DrawTextW(hdc, buf.as_mut_ptr(), buf.len() as i32, &mut rc, DT_SINGLELINE | DT_VCENTER | align);
    SelectObject(hdc, old);
}

/// 简易矢量图标（画线条，不用字体字形），中心 (cx,cy)
unsafe fn draw_glyph(hdc: HDC, id: u16, cx: i32, cy: i32, col: u32) {
    let pen = CreatePen(PS_SOLID as i32, 2, c(col));
    let old_pen = SelectObject(hdc, pen as _);
    let null_brush = GetStockObject(NULL_BRUSH as i32);
    let old_brush = SelectObject(hdc, null_brush);
    match id {
        ID_KEY_BACK => {
            MoveToEx(hdc, cx + 6, cy, ptr::null_mut()); LineTo(hdc, cx - 6, cy);
            MoveToEx(hdc, cx - 6, cy, ptr::null_mut()); LineTo(hdc, cx - 1, cy - 5);
            MoveToEx(hdc, cx - 6, cy, ptr::null_mut()); LineTo(hdc, cx - 1, cy + 5);
        }
        ID_KEY_HOME => {
            MoveToEx(hdc, cx - 7, cy + 1, ptr::null_mut()); LineTo(hdc, cx, cy - 7); LineTo(hdc, cx + 7, cy + 1);
            let body = RECT { left: cx - 5, top: cy + 1, right: cx + 5, bottom: cy + 7 };
            Rectangle(hdc, body.left, body.top, body.right, body.bottom);
        }
        ID_KEY_RECENTS => {
            let r1 = RECT { left: cx - 3, top: cy - 8, right: cx + 7, bottom: cy + 6 };
            let r2 = RECT { left: cx - 7, top: cy - 4, right: cx + 3, bottom: cy + 8 };
            RoundRect(hdc, r1.left, r1.top, r1.right, r1.bottom, 4, 4);
            RoundRect(hdc, r2.left, r2.top, r2.right, r2.bottom, 4, 4);
        }
        ID_KEY_VOL_UP | ID_KEY_VOL_DOWN => {
            // 小喇叭：矩形 + 梯形（描边线稿），右侧 +/-
            Rectangle(hdc, cx - 8, cy - 2, cx - 4, cy + 2);
            let trap: [winapi::shared::windef::POINT; 4] = [
                winapi::shared::windef::POINT { x: cx - 4, y: cy - 2 },
                winapi::shared::windef::POINT { x: cx + 2, y: cy - 6 },
                winapi::shared::windef::POINT { x: cx + 2, y: cy + 6 },
                winapi::shared::windef::POINT { x: cx - 4, y: cy + 2 },
            ];
            Polygon(hdc, trap.as_ptr(), 4);
            MoveToEx(hdc, cx + 5, cy - 1, ptr::null_mut());
            LineTo(hdc, cx + 12, cy - 1);
            if id == ID_KEY_VOL_UP {
                MoveToEx(hdc, cx + 8, cy - 4, ptr::null_mut());
                LineTo(hdc, cx + 8, cy + 2);
            }
        }
        ID_KEY_POWER => {
            Ellipse(hdc, cx - 6, cy - 5, cx + 6, cy + 7);
            MoveToEx(hdc, cx, cy - 8, ptr::null_mut()); LineTo(hdc, cx, cy + 1);
        }
        ID_KEY_NOTIF => {
            // 铃铛：圆顶 + 梯形身 + 横杠 + 小圆点（描边线稿）
            Ellipse(hdc, cx - 4, cy - 9, cx + 4, cy - 1);
            let body: [winapi::shared::windef::POINT; 4] = [
                winapi::shared::windef::POINT { x: cx - 4, y: cy - 4 },
                winapi::shared::windef::POINT { x: cx - 7, y: cy + 5 },
                winapi::shared::windef::POINT { x: cx + 7, y: cy + 5 },
                winapi::shared::windef::POINT { x: cx + 4, y: cy - 4 },
            ];
            Polygon(hdc, body.as_ptr(), 4);
            MoveToEx(hdc, cx - 9, cy + 5, ptr::null_mut());
            LineTo(hdc, cx + 9, cy + 5);
            Ellipse(hdc, cx - 2, cy + 7, cx + 2, cy + 11);
        }
        ID_KEY_SCREENSHOT => {
            let w = 7; let t = 5;
            MoveToEx(hdc, cx - w, cy - t + 4, ptr::null_mut()); LineTo(hdc, cx - w, cy - t); LineTo(hdc, cx - w + 4, cy - t);
            MoveToEx(hdc, cx + w, cy - t + 4, ptr::null_mut()); LineTo(hdc, cx + w, cy - t); LineTo(hdc, cx + w - 4, cy - t);
            MoveToEx(hdc, cx - w, cy + t - 4, ptr::null_mut()); LineTo(hdc, cx - w, cy + t); LineTo(hdc, cx - w + 4, cy + t);
            MoveToEx(hdc, cx + w, cy + t - 4, ptr::null_mut()); LineTo(hdc, cx + w, cy + t); LineTo(hdc, cx + w - 4, cy + t);
            Ellipse(hdc, cx - 3, cy - 3, cx + 3, cy + 3);
        }
        _ => {}
    }
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(pen as _);
}

unsafe fn paint_panel(hwnd: HWND, hdc: HDC) {
    // 背景
    let bg = CreateSolidBrush(c(C_BG));
    let full = RECT { left: 0, top: 0, right: PANEL_W, bottom: PANEL_H };
    FillRect(hdc, &full, bg);
    DeleteObject(bg as _);

    // 标题
    draw_text(hdc, FONT_TITLE.load(Ordering::Relaxed), (16, 10, 150, 40), "投屏控制", C_TEXT, DT_LEFT);
    let serial = SER_TEXT.lock().unwrap().clone();
    draw_text(hdc, FONT_LABEL.load(Ordering::Relaxed), (16, 34, 160, 52), &format!("桌链 · {serial}"), C_DIM, DT_LEFT);

    // 关闭按钮
    let (cl, ct, cr, cb) = close_rect();
    let close_col = if *HOVER_BTN.lock().unwrap() == ID_CLOSE { C_CLOSE } else { C_CARD };
    let cbb = CreateSolidBrush(c(close_col));
    let oldb = SelectObject(hdc, cbb as _);
    let no_pen = GetStockObject(NULL_PEN as i32);
    SelectObject(hdc, no_pen);
    Ellipse(hdc, cl, ct, cr, cb);
    SelectObject(hdc, oldb);
    DeleteObject(cbb as _);
    let xpen = CreatePen(PS_SOLID as i32, 2, c(C_TEXT));
    let oldp = SelectObject(hdc, xpen as _);
    let ccx = (cl + cr) / 2; let ccy = (ct + cb) / 2;
    MoveToEx(hdc, ccx - 5, ccy - 5, ptr::null_mut()); LineTo(hdc, ccx + 5, ccy + 5);
    MoveToEx(hdc, ccx + 5, ccy - 5, ptr::null_mut()); LineTo(hdc, ccx - 5, ccy + 5);
    SelectObject(hdc, oldp);
    DeleteObject(xpen as _);

    // 按钮卡片
    let hover = *HOVER_BTN.lock().unwrap();
    let press = *PRESS_BTN.lock().unwrap();
    for (i, (id, label)) in BUTTONS.iter().enumerate() {
        let (l, t, r, b) = btn_rect(i);
        let fill = if press == *id { C_CARD_PRESS } else if hover == *id { C_CARD_HOV } else { C_CARD };
        let fbr = CreateSolidBrush(c(fill));
        let oldfb = SelectObject(hdc, fbr as _);
        let spb = CreatePen(PS_SOLID as i32, 1, c(C_STROKE));
        let oldsp = SelectObject(hdc, spb as _);
        RoundRect(hdc, l, t, r, b, 14, 14);
        SelectObject(hdc, oldsp);
        DeleteObject(spb as _);
        SelectObject(hdc, oldfb);
        DeleteObject(fbr as _);
        draw_glyph(hdc, *id, l + 26, (t + b) / 2, if press == *id { C_TEXT } else { C_ACCENT });
        draw_text(hdc, FONT_LABEL.load(Ordering::Relaxed), (l + 44, t, r - 8, b), label, C_TEXT, DT_LEFT);
    }

    // 底部提示
    draw_text(hdc, FONT_LABEL.load(Ordering::Relaxed), (14, PANEL_H - 34, PANEL_W - 14, PANEL_H - 12),
        "按键直达手机 · × 断开", C_DIM, DT_CENTER);
    let _ = hwnd;
}

unsafe extern "system" fn panel_wnd_proc(
    hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint_panel(hwnd, hdc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => {
            let mut pt = winapi::shared::windef::POINT {
                x: (lparam & 0xffff) as i16 as i32,
                y: ((lparam >> 16) & 0xffff) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut pt);
            if pt.y <= 46 && !in_close(pt.x, pt.y) {
                return HTCAPTION as LRESULT;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEMOVE => {
            let x = (lparam & 0xffff) as i16 as i32;
            let y = ((lparam >> 16) & 0xffff) as i16 as i32;
            let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
            tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
            tme.dwFlags = TME_LEAVE;
            tme.hwndTrack = hwnd;
            TrackMouseEvent(&mut tme);
            let id = if in_close(x, y) {
                ID_CLOSE
            } else {
                btn_index_at(x, y).map(|i| BUTTONS[i].0).unwrap_or(0)
            };
            let mut hov = HOVER_BTN.lock().unwrap();
            if *hov != id {
                *hov = id;
                drop(hov);
                InvalidateRect(hwnd, ptr::null(), 1);
            }
            0
        }
        WM_MOUSELEAVE => {
            *HOVER_BTN.lock().unwrap() = 0;
            *PRESS_BTN.lock().unwrap() = 0;
            InvalidateRect(hwnd, ptr::null(), 1);
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xffff) as i16 as i32;
            let y = ((lparam >> 16) & 0xffff) as i16 as i32;
            if in_close(x, y) {
                let _ = crate::mirror::stop();
                DestroyWindow(hwnd);
                return 0;
            }
            if let Some(i) = btn_index_at(x, y) {
                let id = BUTTONS[i].0;
                if let Some(code) = keyevent_for(id) {
                    if let Err(e) = crate::mirror::send_keyevent(code) {
                        log::error!("panel keyevent {code} failed: {e:#}");
                    }
                }
                *PRESS_BTN.lock().unwrap() = id;
                InvalidateRect(hwnd, ptr::null(), 1);
            }
            0
        }
        WM_LBUTTONUP => {
            if *PRESS_BTN.lock().unwrap() != 0 {
                *PRESS_BTN.lock().unwrap() = 0;
                InvalidateRect(hwnd, ptr::null(), 1);
            }
            0
        }
        WM_MOUSEWHEEL => 0,
        WM_DESTROY => {
            PANEL_HWND.store(0, Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub const SYNC_TIMER_ID: UINT = SYNC_TIMER;
