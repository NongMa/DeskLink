use axum::{
    extract::{ConnectInfo, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Deserialize)]
struct ClientMessage {
    #[serde(rename = "type")]
    msg_type: String,
    action: Option<String>,
    // 鼠标控制字段
    dx: Option<i32>,
    dy: Option<i32>,
    button: Option<String>,
    state: Option<String>,
    wheel: Option<i32>,
    // 手机端 ASR 识别出的文字（type = "voice_command"）
    text: Option<String>,
    // 手机上报的设备名（type = "hello"，用于管理端「已连接设备」展示）
    name: Option<String>,
}

/// 转义字符串中的 JSON 特殊字符，避免把识别结果直接拼进 JSON 导致解析失败
fn json_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            c => vec![c],
        })
        .collect()
}

pub async fn start_ws_gateway(
    listener: tokio::net::TcpListener,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/admin", get(crate::admin::admin_page))
        .route("/admin/api/status", get(crate::admin::api_status))
        .route("/admin/api/logs", get(crate::admin::api_logs))
        .route("/admin/api/logs/clear", post(crate::admin::api_logs_clear))
        .route("/admin/api/pair", get(crate::admin::api_pair))
        .route("/admin/api/pair/regenerate", post(crate::admin::api_pair_regenerate))
        .route("/admin/api/clients", get(crate::admin::api_clients))
        .route("/admin/api/clients/:id/kick", post(crate::admin::api_kick))
        .route("/admin/api/mirror/devices", get(crate::admin::api_mirror_devices))
        .route("/admin/api/mirror/connect", post(crate::admin::api_mirror_connect))
        .route("/admin/api/mirror/disconnect", post(crate::admin::api_mirror_disconnect))
        .route("/admin/api/mirror/tcpip", post(crate::admin::api_mirror_tcpip))
        .route("/admin/api/mirror/status", get(crate::admin::api_mirror_status))
        .route("/admin/api/mirror/start", post(crate::admin::api_mirror_start))
        .route("/admin/api/mirror/stop", post(crate::admin::api_mirror_stop))
        .route("/admin/api/mirror/key", post(crate::admin::api_mirror_key))
        .route("/admin/api/fileman/list", get(crate::admin::api_fileman_list))
        .route("/admin/api/fileman/gallery", get(crate::admin::api_fileman_gallery))
        .route("/admin/api/fileman/mkdir", post(crate::admin::api_fileman_mkdir))
        .route("/admin/api/fileman/rename", post(crate::admin::api_fileman_rename))
        .route("/admin/api/fileman/delete", post(crate::admin::api_fileman_delete))
        .route("/admin/api/fileman/download", get(crate::admin::api_fileman_download))
        .route("/admin/api/fileman/raw", get(crate::admin::api_fileman_raw))
        .route("/admin/api/fileman/thumb", get(crate::admin::api_fileman_thumb))
        .route(
            "/admin/api/fileman/upload",
            post(crate::admin::api_fileman_upload).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route(
            "/admin/api/settings",
            get(crate::admin::api_settings_get).post(crate::admin::api_settings_post),
        )
        .with_state(state);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn ws_handler(
    State(st): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    // 配对令牌认证（管理页可关闭；关闭后与旧版行为一致）
    if st.auth_required() {
        let ok = query
            .get("token")
            .map(|t| t == &st.token())
            .unwrap_or(false);
        if !ok {
            log::warn!("rejected unauthenticated WS connection from {peer}");
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    }
    ws.on_upgrade(move |mut socket| async move {
        let conn = st.register_client(peer.to_string());
        log::info!("client #{} connected from {}", conn.id, conn.peer);
        let kill = conn.kill.clone();
        loop {
            tokio::select! {
                _ = kill.notified() => {
                    log::info!("client #{} kicked by admin", conn.id);
                    let _ = socket.send(axum::extract::ws::Message::Text(
                        r#"{"type":"error","message":"已被管理员踢出"}"#.into(),
                    )).await;
                    break;
                }
                msg = socket.next() => {
                    let Some(Ok(msg)) = msg else { break };
                    if let axum::extract::ws::Message::Text(text) = msg {
                        if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                            match parsed.msg_type.as_str() {
                                "hello" => {
                                    if let Some(name) = parsed.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                                        st.set_client_name(conn.id, name.to_string());
                                        log::info!("client #{} identified as '{}' (from {})", conn.id, name, conn.peer);
                                    }
                                }
                                "mouse" => {
                                    if let (Some(dx), Some(dy)) = (parsed.dx, parsed.dy) {
                                        crate::input::mouse_move(dx, dy);
                                    } else if let Some(button) = parsed.button.as_deref() {
                                        let r = match parsed.state.as_deref() {
                                            Some("down") => crate::input::mouse_button(button, true),
                                            Some("up") => crate::input::mouse_button(button, false),
                                            _ => crate::input::mouse_click(button), // 默认单击
                                        };
                                        if let Err(e) = r {
                                            log::error!("mouse button failed: {e:#}");
                                        }
                                    } else if let Some(w) = parsed.wheel {
                                        crate::input::mouse_wheel(w);
                                    }
                                }
                                "focus_check" => {
                                    // 语音模式联动：判断 PC 当前是否有输入框获得焦点（超时降级为 false）
                                    let focused = tokio::time::timeout(
                                        std::time::Duration::from_secs(3),
                                        tokio::task::spawn_blocking(crate::input::foreground_input_focused),
                                    )
                                    .await
                                    .ok()
                                    .and_then(|r| r.ok())
                                    .unwrap_or(false);
                                    log::info!("focus_check -> input_focused={}", focused);
                                    let reply = format!(r#"{{"type":"focus_state","input":{}}}"#, focused);
                                    let _ = socket.send(axum::extract::ws::Message::Text(reply.into())).await;
                                }
                                "input_text" => {
                                    // 手机端识别为文字后，直接打进 PC 当前焦点输入框
                                    let text = parsed.text.clone().unwrap_or_default();
                                    let ok = !text.trim().is_empty();
                                    if ok {
                                        log::info!("input_text: {} chars", text.chars().count());
                                        crate::input::type_text(&text);
                                    } else {
                                        log::warn!("input_text: empty text, ignored");
                                    }
                                    let reply = format!(
                                        r#"{{"type":"input_result","ok":{},"text":"{}"}}"#,
                                        ok,
                                        json_escape(&text)
                                    );
                                    let _ = socket.send(axum::extract::ws::Message::Text(reply.into())).await;
                                }
                                "control" => {
                                    if let Some(action) = parsed.action {
                                        log::info!("control action: {}", action);
                                        if let Err(e) = crate::input::exec_action(&action) {
                                            log::error!("action failed: {}", e);
                                        }
                                    }
                                }
                                "voice_command" => {
                                    // 手机端已完成识别：直接用识别文字执行指令
                                    let text = parsed.text.clone().unwrap_or_default().trim().to_string();
                                    log::info!("voice_command (phone asr): {text}");
                                    let action = crate::audio::parse_and_execute(&text);
                                    let reply = match action {
                                        Some(a) => format!(
                                            r#"{{"type":"command","action":"{a}","text":"{}"}}"#,
                                            json_escape(&text)
                                        ),
                                        None => format!(
                                            r#"{{"type":"command","action":"none","text":"{}"}}"#,
                                            json_escape(&text)
                                        ),
                                    };
                                    let _ = socket.send(axum::extract::ws::Message::Text(reply)).await;
                                }
                                "ping" => {
                                    let _ = socket.send(axum::extract::ws::Message::Text(
                                        r#"{"type":"pong"}"#.into(),
                                    )).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        st.unregister_client(conn.id);
        log::info!("client #{} disconnected ({} left)", conn.id, st.clients().len());
    })
}
