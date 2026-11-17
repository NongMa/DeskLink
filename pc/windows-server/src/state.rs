//! 服务端共享状态：配置快照 + 已连接设备 + Whisper 就绪标记

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// whisper 状态：0=加载中/未开始, 1=就绪, 2=失败
pub static WHISPER_STATE: AtomicU8 = AtomicU8::new(0);

pub fn whisper_state() -> u8 {
    WHISPER_STATE.load(Ordering::Relaxed)
}

#[derive(Clone)]
pub struct ClientConn {
    pub id: u64,
    pub peer: String,
    pub connected: Instant,
    /// 通知该连接退出读取循环（踢出用）
    pub kill: Arc<tokio::sync::Notify>,
    /// 手机 hello 消息上报的设备名（空串 = 未上报，管理端回落显示 IP）
    pub device_name: Arc<Mutex<String>>,
}

pub struct Shared {
    pub token: String,
    pub auth_required: bool,
    device_name: String,
    clients: HashMap<u64, ClientConn>,
}

pub struct AppState {
    pub shared: Mutex<Shared>,
    pub lan_ip: String,
    pub port: u16,
    pub start: Instant,
    pub version: &'static str,
}

static CLIENT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl AppState {
    pub fn new(token: String, auth_required: bool, port: u16, device_name: String) -> Arc<Self> {
        let device_name = {
            let t = device_name.trim().to_string();
            if t.is_empty() {
                crate::config::default_device_name()
            } else {
                t
            }
        };
        Arc::new(AppState {
            shared: Mutex::new(Shared {
                token,
                auth_required,
                device_name,
                clients: HashMap::new(),
            }),
            lan_ip: lan_ip_guess(),
            port,
            start: Instant::now(),
            version: env!("CARGO_PKG_VERSION"),
        })
    }

    pub fn register_client(&self, peer: String) -> ClientConn {
        let id = CLIENT_ID.fetch_add(1, Ordering::Relaxed);
        let conn = ClientConn {
            id,
            peer,
            connected: Instant::now(),
            kill: Arc::new(tokio::sync::Notify::new()),
            device_name: Arc::new(Mutex::new(String::new())),
        };
        self.shared.lock().unwrap().clients.insert(id, conn.clone());
        conn
    }

    pub fn unregister_client(&self, id: u64) {
        self.shared.lock().unwrap().clients.remove(&id);
    }

    pub fn clients(&self) -> Vec<ClientConn> {
        let mut v: Vec<_> = self.shared.lock().unwrap().clients.values().cloned().collect();
        v.sort_by_key(|c| c.connected);
        v
    }

    pub fn set_client_name(&self, id: u64, name: String) -> bool {
        let s = self.shared.lock().unwrap();
        match s.clients.get(&id) {
            Some(c) => {
                *c.device_name.lock().unwrap() = name;
                true
            }
            None => false,
        }
    }

    pub fn kick_client(&self, id: u64) -> bool {
        if let Some(c) = self.shared.lock().unwrap().clients.get(&id) {
            c.kill.notify_one();
            true
        } else {
            false
        }
    }

    pub fn auth_required(&self) -> bool {
        self.shared.lock().unwrap().auth_required
    }

    pub fn set_auth_required(&self, v: bool) {
        self.shared.lock().unwrap().auth_required = v;
    }

    pub fn token(&self) -> String {
        self.shared.lock().unwrap().token.clone()
    }

    pub fn device_name(&self) -> String {
        self.shared.lock().unwrap().device_name.clone()
    }

    pub fn set_device_name(&self, v: String) {
        let v = v.trim().to_string();
        if v.is_empty() {
            return;
        }
        self.shared.lock().unwrap().device_name = v;
    }

    pub fn regenerate_token(&self) -> String {
        let t = crate::config::generate_token();
        self.shared.lock().unwrap().token = t.clone();
        // 写回配置文件
        let mut cfg = crate::config::load_or_create();
        cfg.token = t.clone();
        cfg.auth_required = self.auth_required();
        cfg.device_name = self.device_name();
        crate::config::save(&cfg);
        t
    }

    pub fn persist(&self) {
        let (token, auth_required, device_name) = {
            let s = self.shared.lock().unwrap();
            (s.token.clone(), s.auth_required, s.device_name.clone())
        };
        crate::config::save(&crate::config::Config { token, auth_required, device_name });
    }

    pub fn pair_link(&self) -> String {
        format!(
            "vrc://pair?host={}&port={}&token={}&name={}",
            self.lan_ip,
            self.port,
            self.token(),
            percent_encode(&self.device_name())
        )
    }
}

/// 不发送数据包，仅用 UDP socket 选出默认路由对应的本机地址
/// 仅保留未保留（unreserved）字符，其余按 UTF-8 字节做 percent-encode
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn lan_ip_guess() -> String {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip = addr.ip().to_string();
                if ip != "0.0.0.0" {
                    return ip;
                }
            }
        }
    }
    "127.0.0.1".to_string()
}
