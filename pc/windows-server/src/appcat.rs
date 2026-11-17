//! 动态应用目录：运行时扫描本机已安装应用（开始菜单 / 桌面快捷方式 + UWP 应用），
//! 生成「显示名 → 启动目标」映射，供「打开<应用>」语音指令查找，取代写死的别名表。
//!
//! 查找顺序：精确名/exe 词干 → 前缀匹配 → 子串匹配（各层取名字最短者，降低歧义，如
//! 「微信」命中的是「微信」而不是「微信web开发者工具」）。
//!
//! 目录在服务器启动时于后台线程构建并缓存到 app_catalog.json（7 天过期），
//! 首次语音指令命中时也可同步兜底构建，避免未构建时误判「找不到应用」。
//!
//! 安全：扫描只读快捷方式，绝不执行；名称里含「卸载/uninstall/修复/repair」的
//! 卸载入口一律过滤掉，防止语音误触发卸载器。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct Entry {
    target: String,
    name_norm: String,
    stem_norm: String,
}

/// PowerShell 扫描环境（开始菜单两处 + 公共/个人桌面 + Get-StartApps UWP 清单）
const SCAN_SCRIPT: &str = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$ErrorActionPreference = 'SilentlyContinue'
$rows = New-Object System.Collections.Generic.List[object]
$sh = New-Object -ComObject WScript.Shell
$roots = @(
  "$env:ProgramData\Microsoft\Windows\Start Menu\Programs",
  "$env:AppData\Microsoft\Windows\Start Menu\Programs",
  "$env:Public\Desktop",
  "$env:UserProfile\Desktop"
)
foreach ($r in $roots) {
  if (-not (Test-Path -LiteralPath $r)) { continue }
  Get-ChildItem -LiteralPath $r -Recurse -Include '*.lnk','*.url' | ForEach-Object {
    $n = $_.BaseName
    if ($n -match '卸载|uninstall|uninst|删除|修复|repair|modify') { return }
    try {
      if ($_.Extension -eq '.url') {
        $t = (Get-Content -LiteralPath $_.FullName | Where-Object { $_ -match '^URL=' }) -replace '^URL=',''
      } else {
        $t = $sh.CreateShortcut($_.FullName).TargetPath
      }
      if ($t) { $rows.Add([pscustomobject]@{ name = $n; target = $t }) }
    } catch {}
  }
}
try { Get-StartApps | ForEach-Object {
  if ($_.Name -match '卸载|uninstall|删除|修复') { return }
  $rows.Add([pscustomobject]@{ name = $_.Name; target = "shell:AppsFolder\$($_.AppID)" })
} } catch {}
$rows | ConvertTo-Json -Compress -Depth 3
"#;

static CATALOG: OnceLock<Vec<Entry>> = OnceLock::new();
static BUILD_LOCK: Mutex<()> = Mutex::new(());

fn cache_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("app_catalog.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 规范化：转小写、去空白与常见标点，保留中英文及数字（用于匹配）
fn norm(s: &str) -> String {
    const DROP: &[char] = &[
        ' ', '　', '-', '_', '·', '(', ')', '（', '）', '[', ']', '【', '】', '。', '，', ',',
        '.', '、', '/', '\\', '–', '—', '+', '&', '＆', '！', '!',
    ];
    s.to_lowercase().chars().filter(|c| !DROP.contains(c)).collect()
}

fn stem_norm(target: &str) -> String {
    // shell:AppsFolder\AUMID！应用 → 词干直接用整个 AppID；否则取 exe 文件名词干
    if let Some(id) = target.strip_prefix("shell:AppsFolder\\") {
        return norm(id);
    }
    Path::new(target)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(norm)
        .unwrap_or_default()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    built_at: u64,
    entries: Vec<serde_json::Value>,
}

fn save_cache(entries: &[Entry]) {
    let values: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| serde_json::json!({ "name": e.name_norm, "target": e.target, "stem": e.stem_norm }))
        .collect();
    let text = match serde_json::to_string(&CacheFile {
        built_at: now_secs(),
        entries: values,
    }) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("app catalog: cache serialize failed: {e:#}");
            return;
        }
    };
    if let Err(e) = std::fs::write(cache_path(), text) {
        log::warn!("app catalog: cache write failed: {e:#}");
    }
}

fn load_cache() -> Option<Vec<Entry>> {
    let text = std::fs::read_to_string(cache_path()).ok()?;
    let cache: CacheFile = serde_json::from_str(&text).ok()?;
    if now_secs().saturating_sub(cache.built_at) > 7 * 24 * 3600 {
        log::info!("app catalog: cache older than 7d, rescanning");
        return None;
    }
    let entries: Vec<Entry> = cache
        .entries
        .iter()
        .filter_map(|v| {
            Some(Entry {
                target: v["target"].as_str()?.to_string(),
                name_norm: v["name"].as_str()?.to_string(),
                stem_norm: v["stem"].as_str()?.to_string(),
            })
        })
        .collect();
    if entries.is_empty() {
        None
    } else {
        log::info!("app catalog: loaded {} entries from cache", entries.len());
        Some(entries)
    }
}

fn scan_powershell() -> anyhow::Result<Vec<Entry>> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCAN_SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !output.status.success() {
        anyhow::bail!("powershell scan exited with {}", output.status);
    }
    let json = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(json.trim())
        .map_err(|e| anyhow::anyhow!("scan output is not valid JSON: {e}"))?;
    let rows: Vec<serde_json::Value> = match value {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![value],
        _ => vec![],
    };
    const DROP_NAME: &[&str] = &["卸载", "uninstall", "uninst", "删除", "修复", "repair", "modify"];
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(name) = row["name"].as_str() else { continue };
        let Some(target) = row["target"].as_str() else { continue };
        let name = name.trim().to_string();
        let target = target.trim().to_string();
        if name.is_empty() || target.is_empty() {
            continue;
        }
        let lower = name.to_lowercase();
        if DROP_NAME.iter().any(|k| lower.contains(k)) {
            continue;
        }
        // Get-StartApps 会把桌面应用输出成 AppID="完整路径" 的假 shell:AppsFolder 目标，
        // 真正的 AUMID 形如 MicrosoftWindows.Client.Core_xxx!AppName（含 '!')，否则丢弃
        if let Some(id) = target.strip_prefix("shell:AppsFolder\\") {
            if !id.contains('!') {
                continue;
            }
        }
        out.push(Entry {
            stem_norm: stem_norm(&target),
            name_norm: norm(&name),
            target,
        });
    }
    // 去重（快捷方式与 UWP 清单常列出同一应用）
    let mut seen = std::collections::HashSet::new();
    out.retain(|e| seen.insert((e.name_norm.clone(), e.target.clone())));
    Ok(out)
}

/// 取目录（先命中缓存，否则扫描并写缓存）；带锁防止并发重复扫描
fn catalog() -> Vec<Entry> {
    let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    load_cache().unwrap_or_else(|| match scan_powershell() {
        Ok(entries) => {
            log::info!("app catalog: scanned {} entries from this PC", entries.len());
            save_cache(&entries);
            entries
        }
        Err(e) => {
            log::error!("app catalog: scan failed: {e:#}");
            Vec::new()
        }
    })
}

/// 启动时调用（后台线程）：提前预热目录
pub fn warmup_async() {
    std::thread::Builder::new()
        .name("app-catalog-warmup".into())
        .spawn(|| {
            CATALOG.get_or_init(catalog);
        })
        .map(|_| ())
        .unwrap_or_else(|e| log::warn!("app catalog: warmup thread spawn failed: {e}"));
}

/// 按名称查找启动目标（返回全路径或 shell:AppsFolder\AppID）
pub fn lookup(name: &str) -> Option<String> {
    let q = norm(name);
    if q.chars().count() < 2 {
        return None;
    }
    let entries = CATALOG.get_or_init(catalog);

    // 第 1 层：显示名/词干 精确匹配
    if let Some(e) = entries
        .iter()
        .find(|e| e.name_norm == q || e.stem_norm == q)
    {
        return Some(e.target.clone());
    }
    // 第 2 层：前缀匹配，取词干/名字最短者
    let mut best: Option<&Entry> = None;
    let mut best_len = usize::MAX;
    for e in entries {
        if e.name_norm.starts_with(&q) || e.stem_norm.starts_with(&q) {
            let l = e.name_norm.chars().count();
            if l < best_len {
                best = Some(e);
                best_len = l;
            }
        }
    }
    if let Some(e) = best {
        return Some(e.target.clone());
    }
    // 第 3 层：子串匹配，取词干/名字最短者
    let mut best: Option<&Entry> = None;
    let mut best_len = usize::MAX;
    for e in entries {
        if e.name_norm.contains(&q) || e.stem_norm.contains(&q) {
            let l = e.name_norm.chars().count();
            if l < best_len {
                best = Some(e);
                best_len = l;
            }
        }
    }
    best.map(|e| e.target.clone())
}
