//! 环形日志缓冲 + log crate 适配器。
//! - 继续写 stderr（旧的 launch 脚本会把 stderr 重定向到 logs/server.err.log）
//! - 同时保留最近 N 条结构化日志，供 /admin/api/logs 查询

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;

use serde::Serialize;

const CAPACITY: usize = 2000;

#[derive(Clone, Serialize)]
pub struct LogLine {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub msg: String,
}

static RING: Mutex<VecDeque<LogLine>> = Mutex::new(VecDeque::new());

/// 查询最近 N 条（oldest→newest），可按 level / 关键字过滤
pub fn tail(lines: usize, level: Option<&str>, query: Option<&str>) -> Vec<LogLine> {
    let ring = RING.lock().unwrap();
    let q = query.map(|s| s.to_lowercase());
    let v: Vec<LogLine> = ring
        .iter()
        .filter(|l| level.map_or(true, |lv| l.level == lv))
        .filter(|l| {
            q.as_ref()
                .map_or(true, |s| l.msg.to_lowercase().contains(s) || l.target.to_lowercase().contains(s))
        })
        .cloned()
        .collect();
    let start = v.len().saturating_sub(lines);
    v[start..].to_vec()
}

pub fn clear() {
    RING.lock().unwrap().clear();
    log::info!("logs cleared by admin");
}

fn push(level: log::Level, target: &str, args: &std::fmt::Arguments) {
    let line = LogLine {
        ts: chrono_now(),
        level: level.as_str().to_string(),
        target: target.to_string(),
        msg: format!("{args}"),
    };
    let mut ring = RING.lock().unwrap();
    if ring.len() >= CAPACITY {
        ring.pop_front();
    }
    ring.push_back(line);
}

fn chrono_now() -> String {
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "?".to_string())
}

struct RingLogger;

impl log::Log for RingLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::LevelFilter::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        push(record.level(), record.target(), record.args());
        // 继续输出到 stderr（旧 launch 脚本重定向用）
        let _ = writeln!(
            std::io::stderr(),
            "{} {:5} {} {}",
            chrono_now(),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

static LOGGER: RingLogger = RingLogger;

/// 替换 env_logger：初始化环形日志器
pub fn init() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);
}
