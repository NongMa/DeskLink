#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 桌链 DeskLink 管理端：
// 独立的系统窗口（Windows 原生 WebView2），窗口直接加载本机服务提供的管理页
// http://127.0.0.1:8765/admin（仅回环可访问，局域网 403）。
// 关闭窗口只退出管理界面，不影响托盘后台服务。
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("VoiceRemoteControl 管理端启动失败");
}
