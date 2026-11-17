//! 桌链 DeskLink – Windows 托盘 agent（手机与 Windows 协同）
//! 主线程跑 win32 消息循环（托盘），tokio 运行时在后台线程跑 WS/HTTP 服务。

#![windows_subsystem = "windows"]

mod server;
mod input;
mod audio;
mod logbuf;
mod config;
mod state;
mod tray;
mod autostart;
mod admin;
mod appcat;
mod mirror;
mod panel;
mod fileman;

use std::sync::Arc;
use state::WHISPER_STATE;

const PORT: u16 = 8765;

fn main() {
    logbuf::init();
    let cfg = config::load_or_create();
    let state = state::AppState::new(cfg.token, cfg.auth_required, PORT, cfg.device_name);

    log::info!(
        "VRC tray agent v{} starting on 0.0.0.0:{PORT} (lan_ip={}, auth_required={}, pair_link_present=true, device={})",
        state.version,
        state.lan_ip,
        state.auth_required(),
        state.device_name()
    );

    // 后台预热：动态应用目录（本机快捷方式 + UWP），供「打开<应用>」语音指令查找
    appcat::warmup_async();

    // 后台线程：tokio 运行时（WS 服务 + whisper 预热）
    let server_state = Arc::clone(&state);
    std::thread::Builder::new()
        .name("vrc-runtime".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("failed to build tokio runtime: {e}");
                    std::process::exit(1);
                }
            };
            rt.block_on(async move {
                // 后台预热：加载 whisper 模型并跑一次空音频识别，
                // 把首次加载的卡顿挪到启动期，同时暴露失败/卡死情况
                std::thread::Builder::new()
                    .name("whisper-warmup".to_string())
                    .stack_size(64 * 1024 * 1024)
                    .spawn(|| {
                        let t = std::time::Instant::now();
                        log::info!("whisper pre-warm started");
                        match crate::audio::recognize_pcm_i16(&vec![0i16; 16_000]) {
                            Ok(text) => {
                                WHISPER_STATE.store(1, std::sync::atomic::Ordering::Relaxed);
                                log::info!("whisper pre-warm done in {:?} (text={text:?})", t.elapsed());
                            }
                            Err(e) => {
                                WHISPER_STATE.store(2, std::sync::atomic::Ordering::Relaxed);
                                log::error!("whisper pre-warm failed in {:?}: {e:#}", t.elapsed());
                            }
                        }
                    })
                    .ok();

                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], PORT));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        log::error!("bind {addr} failed: {e}");
                        // 通知托盘线程退出，避免留下无服务的托盘图标
                        std::process::exit(1);
                    }
                };
                log::info!("Listening on {addr}");
                if let Err(e) = server::start_ws_gateway(listener, server_state).await {
                    log::error!("server exited with error: {e:#}");
                    std::process::exit(1);
                }
            });
        })
        .expect("failed to spawn runtime thread");

    // 主线程：托盘图标 + 消息循环
    tray::run(state);
}
