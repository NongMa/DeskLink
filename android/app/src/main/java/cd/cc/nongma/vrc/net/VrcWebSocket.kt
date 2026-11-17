package cd.cc.nongma.vrc.net

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import okhttp3.*
import java.util.concurrent.TimeUnit

import android.content.Context
import android.content.SharedPreferences

enum class ConnectionStatus { Disconnected, Connecting, Connected, Error }

/** Siri 式语音交互的生命周期阶段 */
enum class VoicePhase { Idle, Listening, Processing, Result, Error }

/** 语音识别结果用途：PC 文本输入焦点激活时出现「转文字输入 / 指令识别」双模式 */
enum class VoiceInputMode { Command, Text }

object VrcWebSocket {
    private const val DEFAULT_HOST = "192.168.77.181"
    private const val DEFAULT_PORT = 8765

    private val _status = MutableStateFlow(ConnectionStatus.Disconnected)
    val status: StateFlow<ConnectionStatus> = _status.asStateFlow()

    private val _lastMessage = MutableStateFlow("")
    val lastMessage: StateFlow<String> = _lastMessage.asStateFlow()

    /** 语音交互状态机（Siri 式覆盖层消费这三个值） */
    private val _voicePhase = MutableStateFlow(VoicePhase.Idle)
    val voicePhase: StateFlow<VoicePhase> = _voicePhase.asStateFlow()
    private val _voiceTitle = MutableStateFlow("")
    val voiceTitle: StateFlow<String> = _voiceTitle.asStateFlow()
    private val _voiceSubtitle = MutableStateFlow("")
    val voiceSubtitle: StateFlow<String> = _voiceSubtitle.asStateFlow()
    /** 聆听期间的实时转写（边说边识别，仅在 Listening 阶段有值） */
    private val _voicePartial = MutableStateFlow("")
    val voicePartial: StateFlow<String> = _voicePartial.asStateFlow()

    /** PC 是否处于文本输入焦点状态（focus_check → focus_state 更新） */
    private val _pcInputFocus = MutableStateFlow(false)
    val pcInputFocus: StateFlow<Boolean> = _pcInputFocus.asStateFlow()

    /** 语音识别模式：输入焦点激活时生效，默认 Text（仅由 false→true 转换时自动置回 Text） */
    private val _inputMode = MutableStateFlow(VoiceInputMode.Text)
    val inputMode: StateFlow<VoiceInputMode> = _inputMode.asStateFlow()

    /** 开始聆听（麦克风按下时调用） */
    fun notifyListening() {
        _voicePhase.value = VoicePhase.Listening
        _voiceTitle.value = ""
        _voiceSubtitle.value = ""
        _voicePartial.value = ""
        requestPcFocus() // 聆听期间刷新一次 PC 输入焦点状态，决定语音界面是否显示双模式切换
    }

    /** 更新聆听期间的实时转写（AudioStreamer 部分识别回调驱动） */
    fun setVoicePartial(text: String) {
        _voicePartial.value = text
    }

    /** 查询 PC 是否有输入框获得焦点（回复 focus_state，经 handleServerMessage 更新 _pcInputFocus） */
    fun requestPcFocus() {
        val socket = ws
        if (socket == null || _status.value != ConnectionStatus.Connected) return
        runCatching { socket.send(encodeJson(mapOf("type" to "focus_check"))) }
    }

    /** 手动切换语音识别模式（语音界面双模式切换器）。焦点 false→true 时会被自动重置为 Text */
    fun setInputMode(mode: VoiceInputMode) {
        if (_inputMode.value == mode) return
        _inputMode.value = mode
    }

    /** 松手后进入识别中（无论走手机端还是 PC 端识别都先置为 Processing） */
    fun notifyProcessing() {
        _voicePartial.value = ""
        if (_voicePhase.value == VoicePhase.Listening || _voicePhase.value == VoicePhase.Idle) {
            _voicePhase.value = VoicePhase.Processing
        }
    }

    private fun jsonEscape(s: String): String =
        s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "\\r")

    /** 手机端识别出文字后发给 PC 执行（PC 不再跑 Whisper） */
    fun sendVoiceCommand(text: String) {
        val socket = ws
        if (socket == null || _status.value != ConnectionStatus.Connected) {
            _voicePhase.value = VoicePhase.Error
            _voiceTitle.value = "未连接到电脑"
            _voiceSubtitle.value = "请先连接电脑后再试"
            _lastMessage.value = "错误: 未连接到电脑"
            return
        }
        if (_pcInputFocus.value && _inputMode.value == VoiceInputMode.Text) {
            // PC 有输入焦点且选择「转文字输入」→ 直接打进输入框，绕过指令解析
            socket.send("{\"type\":\"input_text\",\"text\":\"${jsonEscape(text)}\"}")
        } else {
            socket.send("{\"type\":\"voice_command\",\"text\":\"${jsonEscape(text)}\"}")
        }
        if (_voicePhase.value == VoicePhase.Listening) {
            _voicePhase.value = VoicePhase.Processing
        }
    }

    /** 语音会话结束/取消时复位到 Idle */
    fun resetVoiceState() {
        _voicePhase.value = VoicePhase.Idle
        _voiceTitle.value = ""
        _voiceSubtitle.value = ""
        _voicePartial.value = ""
    }

    private fun prettyAction(action: String): String = when (action) {
        "mirror_start" -> "开始投屏"
        "mirror_stop" -> "结束投屏"
        "open_browser", "open_chrome" -> "打开浏览器"
        "open_calc" -> "打开计算器"
        "open_notepad" -> "打开记事本"
        "volume_up" -> "音量加"
        "volume_down" -> "音量减"
        "volume_mute" -> "静音"
        "play_pause", "media_play_pause" -> "播放 / 暂停"
        "next_track" -> "下一曲"
        "prev_track" -> "上一曲"
        "enter", "ok" -> "回车"
        "lock_workstation" -> "锁屏"
        "screenshot" -> "截图到剪贴板"
        "show_desktop" -> "显示桌面"
        "task_view" -> "任务视图"
        "alt_tab", "switch_window" -> "切换窗口"
        "close_window" -> "关闭当前窗口"
        "fullscreen" -> "切换全屏"
        "go_back" -> "后退"
        "go_forward" -> "前进"
        "refresh" -> "刷新"
        "tab_new" -> "新建标签页"
        "tab_close" -> "关闭标签页"
        "tab_next" -> "下一个标签页"
        "open_taskmgr" -> "打开任务管理器"
        "open_explorer" -> "打开资源管理器"
        "open_control" -> "打开控制面板"
        "open_settings" -> "打开设置"
        "open_paint" -> "打开画图"
        else -> if (action.startsWith("open_app:")) "打开 ${action.removePrefix("open_app:")}" else action
    }

    /** 当前连接目标地址（用于 UI 展示，不含设备名） */
    fun address(): String = "$currentHost:$currentPort"

    /** 展示用地址：优先显示 PC 端配置的设备名，例如「SKYNOMAD · 192.168.77.13:8765」 */
    fun displayAddress(): String =
        if (currentDeviceName.isBlank()) address() else "$currentDeviceName · ${address()}"

    /** PC 端配置的设备名称（配对链接下发，可能为空） */
    fun deviceName(): String = currentDeviceName

    private var ws: WebSocket? = null
    private var currentHost: String = DEFAULT_HOST
    private var currentPort: Int = DEFAULT_PORT
    private var currentToken: String = ""
    private var currentDeviceName: String = ""
    private var appContext: Context? = null

    private val prefs: SharedPreferences?
        get() = appContext?.getSharedPreferences("vrc_conn", Context.MODE_PRIVATE)

    /** 加载本地保存的配对配置（首次启动/进程重建时恢复） */
    fun init(context: Context) {
        appContext = context.applicationContext
        val p = prefs ?: return
        currentHost = p.getString("host", DEFAULT_HOST) ?: DEFAULT_HOST
        currentPort = p.getInt("port", DEFAULT_PORT)
        currentToken = p.getString("token", "") ?: ""
        currentDeviceName = p.getString("device_name", "") ?: ""
    }

    private fun saveConn() {
        prefs?.edit()?.apply {
            putString("host", currentHost)
            putInt("port", currentPort)
            putString("token", currentToken)
            putString("device_name", currentDeviceName)
            apply()
        }
    }

    /**
     * 从任意文本中提取 vrc://pair?... 链接并配对（剪贴板兜底用）
     * @return 是否成功
     */
    fun applyPairText(text: String): Boolean {
        val trimmed = text.trim()
        // 提取文本里第一个 vrc://pair? 链接（允许整段粘贴里夹带其它字符）
        val match = Regex("""vrc://pair\?[^\s"'<>]+""").find(trimmed)?.value
            ?: trimmed.takeIf { it.startsWith("vrc://") }
            ?: return false
        return applyPairUri(android.net.Uri.parse(match))
    }

    /**
     * 处理配对深链接：vrc://pair?host=IP&port=8765&token=xxx&name=设备名
     * @return 是否成功解析并保存（成功后自动重连）
     */
    fun applyPairUri(uri: android.net.Uri): Boolean {
        if (uri.scheme != "vrc" || uri.host != "pair") return false
        val host = uri.getQueryParameter("host")?.takeIf { it.isNotBlank() } ?: return false
        val port = uri.getQueryParameter("port")?.toIntOrNull() ?: DEFAULT_PORT
        val token = uri.getQueryParameter("token").orEmpty()
        // name 由 PC 管理页配置（配对链接下发），Uri 解析已自动 percent-decode
        currentDeviceName = uri.getQueryParameter("name")?.trim().orEmpty()
        currentHost = host
        currentPort = port
        currentToken = token
        saveConn()
        _lastMessage.value = "已配对 ${displayAddress()}，正在连接…"
        connect()
        return true
    }

    private fun encodeJson(map: Map<String, String>): String {
        return map.entries.joinToString(
            prefix = "{",
            postfix = "}",
            separator = ","
        ) { (k, v) ->
            "\"${jsonEscape(k)}\":\"${jsonEscape(v)}\""
        }
    }

    private val textField = Regex("\"text\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"")
    private val actionField = Regex("\"action\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"")
    private val messageField = Regex("\"message\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"")

    /** 本机设备名：优先系统「设备名称」(Settings.Global.DEVICE_NAME)，缺省退回 厂商+型号 */
    private fun deviceLabel(): String {
        val fromSettings = runCatching {
            android.provider.Settings.Global.getString(
                appContext?.contentResolver,
                android.provider.Settings.Global.DEVICE_NAME
            )
        }.getOrNull()?.trim().orEmpty()
        if (fromSettings.isNotEmpty()) return fromSettings
        val fallback = listOf(android.os.Build.MANUFACTURER, android.os.Build.MODEL)
            .map { it?.trim().orEmpty() }
            .filter { it.isNotEmpty() }
            .joinToString(" ")
        return fallback.ifEmpty { "Android 手机" }
    }

    private fun unescapeJson(s: String): String =
        s.replace("\\\"", "\"").replace("\\\\", "\\").replace("\\n", "\n")

    private fun field(text: String, regex: Regex): String? =
        regex.find(text)?.groupValues?.get(1)?.let(::unescapeJson)

    private fun handleServerMessage(text: String) {
        when {
            // 音频帧 ACK 不刷新 UI
            text.contains("\"status\":\"ok\"") -> Unit
            // pong 心跳确认，忽略
            text.contains("\"type\":\"pong\"") -> Unit
            // PC 输入焦点状态（来自 focus_check）：仅在 false→true 转换时把模式重置为 Text（默认）
            text.contains("\"type\":\"focus_state\"") -> {
                val focused = Regex("\"input\"\\s*:\\s*(true|false)").find(text)?.groupValues?.get(1) == "true"
                if (focused && !_pcInputFocus.value) {
                    _inputMode.value = VoiceInputMode.Text
                }
                _pcInputFocus.value = focused
            }
            // input_text 注入结果
            text.contains("\"type\":\"input_result\"") -> {
                val ok = Regex("\"ok\"\\s*:\\s*(true|false)").find(text)?.groupValues?.get(1) == "true"
                val echoed = field(text, textField).orEmpty()
                if (ok) {
                    _voicePhase.value = VoicePhase.Result
                    _voiceTitle.value = echoed.ifBlank { "已输入到电脑" }
                    _voiceSubtitle.value = "已输入到电脑输入框"
                    _lastMessage.value = "已输入到电脑输入框: $echoed"
                } else {
                    _voicePhase.value = VoicePhase.Error
                    _voiceTitle.value = "输入失败"
                    _voiceSubtitle.value = "电脑端未能把文字打进输入框"
                    _lastMessage.value = "错误: 输入失败"
                }
            }
            text.contains("\"type\":\"command\"") -> {
                val recognized = field(text, textField).orEmpty()
                val action = field(text, actionField).orEmpty()
                _voicePhase.value = VoicePhase.Result
                _voiceTitle.value = recognized.ifBlank { "（未听清）" }
                _voiceSubtitle.value = when {
                    action == "none" && recognized == "未检测到人声" -> "没有听到声音"
                    action.startsWith("failed:") -> "执行失败：${prettyAction(action.removePrefix("failed:"))}"
                    action == "none" -> "未匹配到指令"
                    action.isEmpty() -> "收到指令"
                    else -> "已执行：${prettyAction(action)}"
                }
                _lastMessage.value = buildString {
                    if (recognized.isNotEmpty()) append("识别: $recognized\n")
                    when (action) {
                        "none" -> append("未匹配到指令")
                        "" -> append("收到指令")
                        else -> append("执行: $action")
                    }
                }
            }
            text.contains("\"type\":\"error\"") -> {
                val message = field(text, messageField).orEmpty()
                _voicePhase.value = VoicePhase.Error
                _voiceTitle.value = "出错了"
                _voiceSubtitle.value = message
                _lastMessage.value = "错误: $message"
            }
            else -> _lastMessage.value = text
        }
    }

    fun connect(host: String = currentHost, port: Int = currentPort) {
        currentHost = host
        currentPort = port
        disconnect()
        _status.value = ConnectionStatus.Connecting
        // 带上配对令牌（PC 端开启令牌认证时必须携带）
        val url = buildString {
            append("ws://$host:$port/ws")
            if (currentToken.isNotBlank()) append("?token=").append(currentToken)
        }
        val request = Request.Builder()
            .url(url)
            .build()

        val client = OkHttpClient.Builder()
            .readTimeout(0, TimeUnit.MILLISECONDS) // 长连
            .pingInterval(30, TimeUnit.SECONDS)   // 30s 一次心跳保活
            .build()

        ws = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                _status.value = ConnectionStatus.Connected
                _lastMessage.value = "已连接到 PC"
                // 上报设备名，管理端「已连接设备」显示用（失败也不影响连接）
                runCatching {
                    webSocket.send(encodeJson(mapOf("type" to "hello", "name" to deviceLabel())))
                }
                webSocket.send(encodeJson(mapOf("type" to "ping")))
                // 探测 PC 输入焦点，决定语音界面是否显示「转文字输入 / 指令识别」双模式
                runCatching {
                    webSocket.send(encodeJson(mapOf("type" to "focus_check")))
                }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                _status.value = ConnectionStatus.Error
                _pcInputFocus.value = false
                _lastMessage.value = if (response?.code == 401) {
                    "连接被拒绝（未配对）。请在电脑托盘打开管理页，手机扫码配对后再试"
                } else {
                    "连接失败: ${t.message}"
                }
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                handleServerMessage(text)
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                _status.value = ConnectionStatus.Disconnected
                _pcInputFocus.value = false
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                _status.value = ConnectionStatus.Disconnected
                _pcInputFocus.value = false
            }
        })
    }

    fun disconnect() {
        ws?.close(1000, "bye")
        ws = null
        _status.value = ConnectionStatus.Disconnected
        _pcInputFocus.value = false
    }

    fun sendControl(action: String) {
        ws?.send(encodeJson(mapOf("type" to "control", "action" to action)))
        _lastMessage.value = "执行: $action"
    }

    fun sendAudioChunk(base64Pcm: String) {
        ws?.send(encodeJson(mapOf("type" to "audio_chunk", "data" to base64Pcm)))
    }

    fun sendMouseMove(dx: Int, dy: Int) {
        ws?.send("{\"type\":\"mouse\",\"dx\":$dx,\"dy\":$dy}")
    }

    fun sendMouseButton(button: String, state: String = "click") {
        ws?.send(encodeJson(mapOf("type" to "mouse", "button" to button, "state" to state)))
    }

    fun sendMouseWheel(delta: Int) {
        ws?.send("{\"type\":\"mouse\",\"wheel\":$delta}")
    }

    /** 回车键（对话框「确定」） */
    fun sendEnter() {
        sendControl("enter")
    }

    fun sendEndOfSpeech() {
        val socket = ws
        if (socket == null || _status.value != ConnectionStatus.Connected) {
            _voicePhase.value = VoicePhase.Error
            _voiceTitle.value = "未连接到电脑"
            _voiceSubtitle.value = "请先连接电脑后再试"
            _lastMessage.value = "错误: 未连接到电脑"
            return
        }
        socket.send(encodeJson(mapOf("type" to "end_of_speech")))
        if (_voicePhase.value == VoicePhase.Listening) {
            _voicePhase.value = VoicePhase.Processing
        }
    }
}
