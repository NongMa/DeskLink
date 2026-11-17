//! 寮€鏈鸿嚜鍚紙HKCU\Software\Microsoft\Windows\CurrentVersion\Run锛?
//! 浣跨敤 winapi 娉ㄥ唽琛?API锛屼笉闇€瑕佺鐞嗗憳鏉冮檺銆?

use anyhow::{bail, Result};
use winapi::shared::minwindef::HKEY;
use winapi::um::winnt::{KEY_READ, KEY_SET_VALUE, REG_SZ};
use winapi::um::winreg::*;

const VALUE_NAME: &str = "VoiceRemoteControl";
const SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn open_key(access: u32) -> Result<HkeyPtr> {
    let mut hkey: HKEY = std::ptr::null_mut();
    let subkey = wide(SUBKEY);
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            access,
            &mut hkey,
        )
    };
    if status != 0 {
        bail!("RegOpenKeyExW failed: {status}");
    }
    Ok(HkeyPtr(hkey))
}

/// 鍖呰 HKEY 淇濊瘉鍏抽棴锛堥伩鍏嶈８鎸囬拡娉勬紡锛?
struct HkeyPtr(HKEY);
impl Drop for HkeyPtr {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn exe_command() -> Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

fn current_value() -> Option<String> {
    let key = open_key(KEY_READ).ok()?;
    let name = wide(&format!("{VALUE_NAME}\0"));
    let mut buf = [0u16; 2048];
    let mut len = (buf.len() * 2) as u32;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut u8,
            &mut len,
        )
    };
    if status != 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..(len as usize) / 2]).trim_matches('\0').to_string())
}

pub fn is_enabled() -> bool {
    let Some(v) = current_value() else {
        return false;
    };
    let Ok(cmd) = exe_command() else {
        return false;
    };
    v.eq_ignore_ascii_case(&cmd)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let key = open_key(KEY_SET_VALUE)?;
    let name = wide(&format!("{VALUE_NAME}\0"));
    if enabled {
        let cmd = exe_command()?;
        let data = wide(&format!("{cmd}\0"));
        let status = unsafe {
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * 2) as u32,
            )
        };
        if status != 0 {
            bail!("RegSetValueExW failed: {status}");
        }
    } else {
        let status = unsafe { RegDeleteValueW(key.0, name.as_ptr()) };
        // 鍊间笉瀛樺湪瑙嗕负鍒犻櫎鎴愬姛
        if status != 0 && status != 2 /* ERROR_FILE_NOT_FOUND */ {
            bail!("RegDeleteValueW failed: {status}");
        }
    }
    Ok(())
}
