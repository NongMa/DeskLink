use std::path::Path;
use std::sync::{Mutex, OnceLock};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use zhconv::Variant;

// Whisper 上下文只加载一次（否则每次识别都要重载大模型，慢且占满 CPU）
static WHISPER: OnceLock<Mutex<WhisperContext>> = OnceLock::new();

fn exe_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn model_path() -> std::path::PathBuf {
    // 默认 small-q5_1；可用环境变量 VRC_WHISPER_MODEL 覆盖（绝对路径，或相对 exe 同级 models/ 的相对路径）
    if let Ok(p) = std::env::var("VRC_WHISPER_MODEL") {
        let path = Path::new(&p);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        let cand = exe_dir().join("models").join(&p);
        if cand.exists() {
            return cand;
        }
        return Path::new(env!("CARGO_MANIFEST_DIR")).join("models").join(&p);
    }
    // 安装版（MSI）：exe 同级 models/；开发版：仓库 models/
    let installed = exe_dir().join("models").join("ggml-small-q5_1.bin");
    if installed.exists() {
        return installed;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("ggml-small-q5_1.bin")
}

fn shared_context() -> anyhow::Result<std::sync::MutexGuard<'static, WhisperContext>> {
    if let Some(ctx) = WHISPER.get() {
        return Ok(ctx.lock().unwrap_or_else(|e| e.into_inner()));
    }
    let path = model_path();
    if !path.exists() {
        anyhow::bail!(
            "Whisper model not found at {}. Place ggml-small-q5_1.bin (or set VRC_WHISPER_MODEL) in the models/ folder.",
            path.display()
        );
    }
    log::info!("loading Whisper model: {}", path.display());
    let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())?;
    log::info!("Whisper model loaded");
    match WHISPER.set(Mutex::new(ctx)) {
        Ok(()) => Ok(WHISPER.get().unwrap().lock().unwrap_or_else(|e| e.into_inner())),
        Err(_) => Ok(WHISPER.get().unwrap().lock().unwrap_or_else(|e| e.into_inner())),
    }
}

pub fn recognize_pcm_i16(samples: &[i16]) -> anyhow::Result<String> {
    let model = shared_context()?;
    let ctx = &*model;
    let mut state = ctx.create_state()?;

    let f32_samples: Vec<f32> = samples.iter().map(|s| (*s as f32) / 32768.0).collect();

    // 编码器按 30 秒（3000 帧）固定计算，裁剪到 audio_ctx 帧可线性降成本；
    // 语音指令一般 < 5 秒（512 帧），可用 VRC_WHISPER_AUDIO_CTX 覆盖
    let audio_ctx: i32 = std::env::var("VRC_WHISPER_AUDIO_CTX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(512);

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(8);
    params.set_audio_ctx(audio_ctx);
    params.set_max_tokens(96); // 语音指令很短，防止静音下幻觉解码空转
    params.set_language(Some("zh"));
    // initial_prompt 用简体中文，压制模型输出繁体字
    params.set_initial_prompt("以下是普通话的语音指令，请用简体中文输出。例如：打开浏览器。");
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, &f32_samples)?;

    let mut text = String::new();
    let segments = state.full_n_segments();
    for i in 0..segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(t) = segment.to_str() {
                text.push_str(t);
            }
        }
    }

    // 兜底：whisper 小模型偶尔仍输出繁体，统一转简体后再返回/匹配指令
    let text = zhconv::zhconv(text.trim(), Variant::ZhCN);

    Ok(text)
}

pub fn parse_and_execute(text: &str) -> Option<String> {
    const DROP_PUNCT: &[char] = &[
        '.', ',', '。', '，', '！', '？', '!', '?', '、', '；', ';', '：', ':', '「', '」',
        '《', '》', '“', '”', '‘', '’', '"', '\'',
    ];
    let t = text
        .replace(|c: char| c.is_whitespace() || DROP_PUNCT.contains(&c), "")
        .to_lowercase();

    // 「打开/启动/运行 <应用名>」——先尝试剥离前缀，未命中再走关键词表
    let open_target = t
        .strip_prefix("打开")
        .or_else(|| t.strip_prefix("启动"))
        .or_else(|| t.strip_prefix("运行"))
        .map(|s| s.to_string());

    let action: String = if let Some(app) = open_target {
        if app.is_empty() {
            return None;
        }
        match app.as_str() {
            "投屏" => "mirror_start".to_string(),
            "浏览器" | "谷歌" | "谷歌浏览器" | "edge" | "chrome" => "open_browser".to_string(),
            "记事本" | "笔记本" | "notepad" => "open_notepad".to_string(),
            "计算器" | "计算机" | "calc" => "open_calc".to_string(),
            "任务管理器" => "open_taskmgr".to_string(),
            "资源管理器" | "文件管理器" | "文件夹" => "open_explorer".to_string(),
            "控制面板" => "open_control".to_string(),
            "设置" | "系统设置" => "open_settings".to_string(),
            "画图" => "open_paint".to_string(),
            _ => format!("open_app:{app}"),
        }
    } else {
        match t.as_str() {
            // 投屏（scrcpy）：voice.startsWith 之外的独立通道，放最前避免被其它关键词截获
            s if s == "投屏" || s.contains("开始投屏") || s.contains("打开投屏") || s.contains("启动投屏") => "mirror_start".to_string(),
            s if s.contains("结束投屏") || s.contains("关闭投屏") || s.contains("停止投屏") || s.contains("关掉投屏") => "mirror_stop".to_string(),
            s if s.contains("音量加") || s.contains("大声") => "volume_up".to_string(),
            s if s.contains("音量减") || s.contains("小声") => "volume_down".to_string(),
            s if s.contains("静音") => "volume_mute".to_string(),
            s if s.contains("浏览器") || s.contains("chrome") || s.contains("谷歌") || s.contains("edge") => "open_browser".to_string(),
            s if s.contains("记事本") || s.contains("笔记本") || s.contains("notepad") => "open_notepad".to_string(),
            s if s.contains("计算器") || s.contains("计算机") || s.contains("calc") => "open_calc".to_string(),
            s if s.contains("播放") || s.contains("暂停") => "play_pause".to_string(),
            s if s.contains("下一曲") || s.contains("下一首") => "next_track".to_string(),
            s if s.contains("上一曲") || s.contains("上一首") => "prev_track".to_string(),
            s if s.contains("任务管理器") => "open_taskmgr".to_string(),
            s if s.contains("资源管理器") || s.contains("文件管理器") => "open_explorer".to_string(),
            s if s.contains("锁屏") || s.contains("锁定电脑") || s.contains("锁屏幕") => "lock_workstation".to_string(),
            s if s.contains("截图") => "screenshot".to_string(),
            s if s.contains("显示桌面") || s.contains("最小化所有") || s.contains("最小化全部") => "show_desktop".to_string(),
            s if s.contains("任务视图") || s.contains("任务切换") => "task_view".to_string(),
            s if s.contains("切换窗口") || s.contains("下一个窗口") || s.contains("换一个窗口") => "alt_tab".to_string(),
            s if s.contains("关闭窗口") || s.contains("关掉窗口") => "close_window".to_string(),
            s if s.contains("全屏") => "fullscreen".to_string(),
            s if s.contains("返回") || s.contains("后退") || s.contains("上一页") => "go_back".to_string(),
            s if s.contains("前进") || s.contains("下一页") => "go_forward".to_string(),
            s if s.contains("刷新") || s.contains("重载") => "refresh".to_string(),
            s if s.contains("新建标签") || s.contains("新标签") => "tab_new".to_string(),
            s if s.contains("关闭标签") => "tab_close".to_string(),
            s if s.contains("下一个标签") || s.contains("切换标签") => "tab_next".to_string(),
            _ => return None,
        }
    };

    match crate::input::exec_action(&action) {
        Ok(()) => Some(action),
        Err(e) => {
            log::error!("action {action} failed: {e:#}");
            Some(format!("failed:{action}"))
        }
    }
}
