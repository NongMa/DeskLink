//! 手机存储文件管理（adb 通道，仅管理页 loopback 可用）
//! - 作用于自动挑选的在线设备（USB 优先，与投屏同一链路）
//! - 路径限制在 /sdcard 内（/storage/emulated/0 自动归一化为 /sdcard）
//! - 下载 / raw 走 `adb exec-out cat` 流式转发，不把整文件读进内存

use std::process::{Command, Stdio};

use serde::Serialize;

use crate::mirror::{adb_exe, connected_serial, CREATE_NO_WINDOW};

#[derive(Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// 日期原文（toybox 两种格式之一，仅展示）
    pub date: String,
}

#[derive(Clone, Serialize)]
pub struct AlbumItem {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub mtime: i64,
    /// "image" | "video"
    pub kind: String,
}

/// 路径归一化：折叠重复斜杠、处理 . 与 ..，并限制在 /sdcard 内
pub fn normalize_path(raw: &str) -> anyhow::Result<String> {
    let p = raw.trim().replace('\\', "/");
    if p.is_empty() {
        anyhow::bail!("路径不能为空");
    }
    if p.chars().any(|c| c.is_control()) {
        anyhow::bail!("路径包含非法字符");
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    anyhow::bail!("路径越界");
                }
            }
            s => parts.push(s),
        }
    }
    let joined = format!("/{}", parts.join("/"));
    if joined == "/sdcard" || joined.starts_with("/sdcard/") {
        return Ok(joined);
    }
    if joined == "/storage/emulated/0" {
        return Ok("/sdcard".to_string());
    }
    if let Some(rest) = joined.strip_prefix("/storage/emulated/0/") {
        return Ok(format!("/sdcard/{rest}"));
    }
    anyhow::bail!("仅允许访问 /sdcard 下的路径")
}

fn sanitize_name(name: &str) -> anyhow::Result<String> {
    let n = name.trim();
    if n.is_empty() {
        anyhow::bail!("名称不能为空");
    }
    if n == "." || n == ".." || n.contains('/') || n.contains('\\') || n.chars().any(|c| c.is_control()) {
        anyhow::bail!("名称包含非法字符：{n}");
    }
    if n.len() > 255 {
        anyhow::bail!("名称过长");
    }
    Ok(n.to_string())
}

/// POSIX 单引号包裹；名字里的单引号按 '\'' 转义
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn run_adb(args: &[&str]) -> anyhow::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    let adb = adb_exe()?;
    let serial = connected_serial()?;
    let out = Command::new(adb)
        .arg("-s")
        .arg(&serial)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(out)
}

/// 执行 `adb shell <script>`，非零退出码时报错（含 stderr/stdout 原文）
fn shell_ok(script: &str) -> anyhow::Result<String> {
    let out = run_adb(&["shell", script])?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let fallback = stdout.trim().to_string();
        anyhow::bail!(if err.is_empty() { fallback } else { err });
    }
    Ok(stdout)
}

fn parent_of(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((p, _)) => p,
        None => "/sdcard",
    }
}

pub fn list(path: &str) -> anyhow::Result<Vec<FileEntry>> {
    let path = normalize_path(path)?;
    let script = format!("cd {} && ls -lA", shell_quote(&path));
    let out = shell_ok(&script)?;
    let mut entries = Vec::new();
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with("total") {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 8 {
            continue;
        }
        let perms = f[0];
        if perms.len() != 10 {
            continue;
        }
        let is_dir = perms.starts_with('d');
        let size: u64 = f[4].parse().unwrap_or(0);
        // 日期两种格式：yyyy-mm-dd HH:MM（→ name 从 7 开始）
        //              Mon dd HH:MM（→ name 从 8 开始）
        let iso_date = f[5].len() == 10 && f[5].as_bytes()[4] == b'-';
        let (date, name) = if iso_date {
            (f[5..7].join(" "), f[7..].join(" "))
        } else {
            if f.len() < 9 {
                continue;
            }
            (f[5..8].join(" "), f[8..].join(" "))
        };
        if name == "." || name == ".." {
            continue;
        }
        entries.push(FileEntry { name, is_dir, size, date });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

pub fn mkdir(dir: &str, name: &str) -> anyhow::Result<()> {
    let dir = normalize_path(dir)?;
    let name = sanitize_name(name)?;
    let target = format!("{}/{}", dir.trim_end_matches('/'), name);
    shell_ok(&format!("mkdir -- {}", shell_quote(&target)))?;
    Ok(())
}

pub fn rename(path: &str, new_name: &str) -> anyhow::Result<()> {
    let path = normalize_path(path)?;
    let name = sanitize_name(new_name)?;
    let parent = parent_of(&path);
    if parent == "/" || parent.is_empty() {
        anyhow::bail!("不能重命名根目录");
    }
    let target = format!("{}/{}", parent, name);
    if target == path {
        return Ok(());
    }
    shell_ok(&format!("mv -- {} {}", shell_quote(&path), shell_quote(&target)))?;
    Ok(())
}

pub fn delete(path: &str) -> anyhow::Result<()> {
    let path = normalize_path(path)?;
    if path == "/sdcard" {
        anyhow::bail!("不能删除根目录");
    }
    shell_ok(&format!("rm -r -- {}", shell_quote(&path)))?;
    Ok(())
}

/// 文件大小（字节）；用于响应头 Content-Length（失败返回 None）
pub fn file_size(path: &str) -> Option<u64> {
    let path = normalize_path(path).ok()?;
    shell_ok(&format!("stat -c %s -- {}", shell_quote(&path))).ok()?.trim().parse().ok()
}

/// 构造 `adb exec-out <script>` 子进程（stdout 原始字节流），调用方负责 wait
pub fn exec_out_command(script: &str) -> anyhow::Result<tokio::process::Command> {
    let adb = adb_exe()?;
    let serial = connected_serial()?;
    let mut cmd = tokio::process::Command::new(adb);
    cmd.arg("-s")
        .arg(&serial)
        .arg("exec-out")
        .arg(script)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(cmd)
}

/// 生成 `cat <path>` 的 exec-out 脚本（带单引号转义）
pub fn cat_script(path: &str) -> anyhow::Result<String> {
    let path = normalize_path(path)?;
    Ok(format!("cat -- {}", shell_quote(&path)))
}

const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "heic", "heif", "dng", "cr2", "nef", "arw"];
const VIDEO_EXT: &[&str] = &["mp4", "3gp", "webm", "mkv", "avi", "mov", "m4v"];

/// 扫描相册目录（DCIM / Pictures / Movies），返回图片+视频（mtime 倒序，最多 1500 条）
pub fn gallery() -> anyhow::Result<Vec<AlbumItem>> {
    let script = r#"find /sdcard/DCIM /sdcard/Pictures /sdcard/Movies -type f -exec stat -c '%s|%Y|%n' {} + 2>/dev/null | head -n 3000"#;
    let out = run_adb(&["shell", script])?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut items = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '|');
        let (Some(size_s), Some(mtime_s), Some(path)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let path = path.trim_end().to_string();
        let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
        let kind = if IMAGE_EXT.contains(&ext.as_str()) {
            "image"
        } else if VIDEO_EXT.contains(&ext.as_str()) {
            "video"
        } else {
            continue;
        };
        let size: u64 = size_s.trim().parse().unwrap_or(0);
        let mtime: i64 = mtime_s.trim().parse().unwrap_or(0);
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        items.push(AlbumItem { path, name, size, mtime, kind: kind.to_string() });
    }
    items.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    items.truncate(1500);
    Ok(items)
}

/// 常见扩展名 → MIME（浏览内联显示用），其余 application/octet-stream
pub fn mime_of(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mov" | "m4v" => "video/quicktime",
        "3gp" => "video/3gpp",
        "pdf" => "application/pdf",
        "txt" => "text/plain; charset=utf-8",
        "apk" => "application/vnd.android.package-archive",
        _ => "application/octet-stream",
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Content-Disposition：inline=false 时附件下载；ASCII 回退名 + UTF-8 filename*
pub fn content_disposition(path: &str, inline: bool) -> String {
    let name = path.rsplit('/').next().unwrap_or("file");
    let fallback: String = name
        .chars()
        .map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' { c } else { '_' })
        .collect();
    let enc = percent_encode(name);
    let disp = if inline { "inline" } else { "attachment" };
    format!("{disp}; filename=\"{fallback}\"; filename*=UTF-8''{enc}")
}

/// adb push <local> <remote>；成功返回 ()
pub fn adb_push(local: &std::path::Path, remote: &str) -> anyhow::Result<()> {
    let dest = normalize_path(remote)?;
    let local_s = local.to_string_lossy().to_string();
    let out = run_adb(&["push", &local_s, &dest])?;
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        anyhow::bail!(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(())
}

/* ===== 相册缩略图（PC 侧按需生成 + 临时目录缓存） ===== */

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 取文件 (size, mtime)；失败返回 None
pub fn stat_size_mtime(path: &str) -> Option<(u64, i64)> {
    let path = normalize_path(path).ok()?;
    let out = shell_ok(&format!("stat -c '%s|%Y' -- {}", shell_quote(&path))).ok()?;
    let mut it = out.trim().split('|');
    let s = it.next()?.trim().parse::<u64>().ok()?;
    let m = it.next()?.trim().parse::<i64>().ok()?;
    Some((s, m))
}

/// 最小 EXIF 方向解析（仅 JPEG APP1）；返回 1..=8，失败回退 1
fn jpeg_orientation(data: &[u8]) -> u8 {
    if data.len() < 4 || &data[0..2] != b"\xff\xd8" {
        return 1;
    }
    let mut i = 2usize;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if len < 2 {
            break;
        }
        if marker == 0xE1 {
            let seg_end = (i + 2 + len).min(data.len());
            let seg = &data[i + 4..seg_end];
            if seg.len() >= 6 && &seg[0..6] == b"Exif\0\0" {
                let tiff = &seg[6..];
                if tiff.len() >= 8 {
                    let le = &tiff[0..2] == b"II";
                    let g16 = |o: usize| -> u16 {
                        if le {
                            u16::from_le_bytes([tiff[o], tiff[o + 1]])
                        } else {
                            u16::from_be_bytes([tiff[o], tiff[o + 1]])
                        }
                    };
                    let g32 = |o: usize| -> u32 {
                        if le {
                            u32::from_le_bytes([tiff[o], tiff[o + 1], tiff[o + 2], tiff[o + 3]])
                        } else {
                            u32::from_be_bytes([tiff[o], tiff[o + 1], tiff[o + 2], tiff[o + 3]])
                        }
                    };
                    if g16(4) == 42 {
                        let ifd0 = g32(8) as usize;
                        if ifd0 + 2 <= tiff.len() {
                            let n = g16(ifd0) as usize;
                            for k in 0..n {
                                let e = ifd0 + 2 + k * 12;
                                if e + 12 > tiff.len() {
                                    break;
                                }
                                if g16(e) == 0x0112 {
                                    return g16(e + 8) as u8;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 2 + len;
    }
    1
}

const THUMB_MAX_SRC: u64 = 48 * 1024 * 1024;
const THUMB_SIDE: u32 = 360;
const THUMB_CACHE_MAX: usize = 1500;

fn thumb_cache_dir() -> Option<std::path::PathBuf> {
    let d = std::env::temp_dir().join("vrc-thumbs");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// 缓存清理：超过上限时删掉最旧的一半
fn prune_thumb_cache(dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    for en in entries.flatten() {
        if let Ok(meta) = en.metadata() {
            if let Ok(t) = meta.modified() {
                files.push((en.path(), t));
            }
        }
    }
    if files.len() <= THUMB_CACHE_MAX {
        return;
    }
    files.sort_by_key(|(_, t)| *t);
    for (p, _) in files.iter().take(files.len() - THUMB_CACHE_MAX / 2) {
        let _ = std::fs::remove_file(p);
    }
}

/// 生成/读取缩略图（JPEG 字节）。格式不支持（HEIF/RAW 等）或文件不存在时返回 Err。
pub fn thumb(path: &str) -> anyhow::Result<Vec<u8>> {
    use std::os::windows::process::CommandExt;
    let path = normalize_path(path)?;
    let (size, mtime) = stat_size_mtime(&path).ok_or_else(|| anyhow::anyhow!("文件不存在"))?;
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    mtime.hash(&mut h);
    let key = h.finish();
    let cache = thumb_cache_dir().ok_or_else(|| anyhow::anyhow!("无法创建缩略图缓存目录"))?;
    let cache_file = cache.join(format!("{key}.jpg"));
    if let Ok(bytes) = std::fs::read(&cache_file) {
        return Ok(bytes);
    }
    if size > THUMB_MAX_SRC {
        anyhow::bail!("源文件过大，跳过缩略图");
    }

    // 读取远端字节（exec-out cat，阻塞）
    let script = cat_script(&path)?;
    let out = Command::new(adb_exe()?)
        .arg("-s")
        .arg(connected_serial()?)
        .arg("exec-out")
        .arg(&script)
        .creation_flags(CREATE_NO_WINDOW)
        .stderr(Stdio::null())
        .output()?;
    if !out.status.success() {
        anyhow::bail!("读取远端文件失败");
    }
    let data = out.stdout;
    if data.is_empty() {
        anyhow::bail!("远端文件为空");
    }

    // decode + EXIF 方向修正 + 缩放 + JPEG 编码
    let img = image::load_from_memory(&data)?;
    let img = match jpeg_orientation(&data) {
        6 => img.rotate90(),
        8 => img.rotate270(),
        3 => img.rotate180(),
        _ => img,
    };
    let thumb = img.thumbnail(THUMB_SIDE, THUMB_SIDE);
    let mut bytes: Vec<u8> = Vec::with_capacity(32 * 1024);
    {
        use image::codecs::jpeg::JpegEncoder;
        use image::ImageEncoder;
        let mut enc = JpegEncoder::new_with_quality(&mut bytes, 80);
        enc.write_image(thumb.as_bytes(), thumb.width(), thumb.height(), thumb.color().into())?;
    }
    prune_thumb_cache(&cache);
    let tmp = cache.join(format!("{key}.tmp"));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &cache_file)?;
    Ok(bytes)
}
