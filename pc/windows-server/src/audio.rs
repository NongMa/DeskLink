//! 语音指令解析与执行：手机端离线识别出的文字 → 动作（键盘 / 鼠标 / 媒体键 / 启动应用）。
//! 语音识别已在手机端完成（sherpa-onnx + SenseVoice），PC 端不再内置识别模型。

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
