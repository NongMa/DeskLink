package cd.cc.nongma.vrc

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import cd.cc.nongma.vrc.audio.AudioStreamer
import cd.cc.nongma.vrc.net.ConnectionStatus
import cd.cc.nongma.vrc.net.VrcWebSocket
import cd.cc.nongma.vrc.net.VoicePhase
import cd.cc.nongma.vrc.net.VoiceInputMode
import cd.cc.nongma.vrc.ui.glass.ConnectionStatusBar
import cd.cc.nongma.vrc.ui.glass.ExpandableGlassSection
import cd.cc.nongma.vrc.ui.glass.LiquidGlassSurface
import cd.cc.nongma.vrc.ui.glass.LiquidGlassToggle
import cd.cc.nongma.vrc.ui.glass.QuickControlGrid
import cd.cc.nongma.vrc.ui.glass.MouseControlPanel
import cd.cc.nongma.vrc.ui.glass.SiriVoiceOverlay
import cd.cc.nongma.vrc.ui.glass.SiriMicButton
import cd.cc.nongma.vrc.ui.glass.ScanPairFab
import cd.cc.nongma.vrc.ui.glass.LightningGlyph
import cd.cc.nongma.vrc.ui.glass.MouseGlyph
import cd.cc.nongma.vrc.ui.theme.VoiceRemoteControlTheme

private enum class RemotePanel { QUICK, MOUSE }

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // 全面屏：允许内容延伸到刘海/挖孔区域（短边方向）
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        // 恢复本地保存的连接配置（配对后重启 App 也能连上）
        VrcWebSocket.init(applicationContext)
        handlePairIntent(intent)
        // 预热端侧识别模型（226MB 首次加载较慢，提前到应用启动时后台加载）
        cd.cc.nongma.vrc.audio.AsrRecognizer.initAsync(applicationContext)
        setContent {
            VoiceRemoteControlTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                    // 内容自行处理系统栏/刘海内边距，Scaffold 不再追加（保证覆盖层真正铺满全屏）
                    contentWindowInsets = WindowInsets()
                ) {
                    RemoteControlScreen(modifier = Modifier.fillMaxSize())
                }
            }
        }
    }

    /** 处理 vrc://pair 配对深链接（扫码 / 浏览器打开 / am start -a VIEW） */
    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        handlePairIntent(intent)
    }

    private fun handlePairIntent(intent: android.content.Intent?) {
        val data = intent?.data ?: return
        if (data.scheme != "vrc") return
        val ok = VrcWebSocket.applyPairUri(data)
        android.widget.Toast.makeText(
            this,
            if (ok) "配对成功：${VrcWebSocket.displayAddress()}"
            else "配对链接无效",
            android.widget.Toast.LENGTH_SHORT,
        ).show()
    }
}

@Composable
fun RemoteControlScreen(modifier: Modifier = Modifier) {
    val status: ConnectionStatus by VrcWebSocket.status.collectAsState()
    val recording: Boolean by AudioStreamer.recording.collectAsState()
    val voicePhase: VoicePhase by VrcWebSocket.voicePhase.collectAsState()
    val voiceTitle: String by VrcWebSocket.voiceTitle.collectAsState()
    val voiceSubtitle: String by VrcWebSocket.voiceSubtitle.collectAsState()
    val voicePartial: String by VrcWebSocket.voicePartial.collectAsState()
    val pcInputFocus: Boolean by VrcWebSocket.pcInputFocus.collectAsState()
    val inputMode: VoiceInputMode by VrcWebSocket.inputMode.collectAsState()
    val micLevel: Float by AudioStreamer.level.collectAsState()
    val context = LocalContext.current
    val micPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        // 授权后用户需要再按一次麦克风开始说话（避免按下-授权后自动录音停不掉）
        if (!granted) {
            // 未授权：保持静默，按钮按下无反应
        }
    }

    // 工具面板：手风琴模式，同一时刻只展开一个；默认全部收起
    var openPanel by remember { mutableStateOf<RemotePanel?>(null) }

    // Siri 式语音交互覆盖层：聆听 / 识别中 / 结果 任一阶段显示
    val overlayVisible = recording || voicePhase != VoicePhase.Idle
    // 覆盖层显示时重新探测 PC 输入焦点（决定语音界面双模式切换器显隐）
    LaunchedEffect(overlayVisible) {
        if (overlayVisible) VrcWebSocket.requestPcFocus()
    }
    val closeVoiceOverlay: () -> Unit = {
        if (AudioStreamer.recording.value) AudioStreamer.cancel()
        else VrcWebSocket.resetVoiceState()
    }
    BackHandler(enabled = overlayVisible, onBack = closeVoiceOverlay)
    val startMic: () -> Unit = {
        val granted = ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) {
            AudioStreamer.start(context)
        } else {
            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    Box(
        modifier = modifier.fillMaxSize()
    ) {
        // 全屏液态玻璃背景
        LiquidGlassSurface(modifier = Modifier.fillMaxSize())

        // 主页内容避开状态栏/导航栏/刘海区域，背景面仍延伸到全屏幕边缘
        Column(
            modifier = Modifier
                .fillMaxSize()
                .safeDrawingPadding()
                .padding(horizontal = 16.dp, vertical = 10.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // ═══ 顶栏：应用标题 + 主题切换 ═══
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 2.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Column {
                    Text(
                        text = "语音遥控",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = "VOICE REMOTE CONTROL",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Spacer(modifier = Modifier.weight(1f))
                LiquidGlassToggle()
                ScanPairFab()
            }

            // ═══ 连接状态条（紧凑重排）═══
            ConnectionStatusBar(
                status = status,
                address = VrcWebSocket.displayAddress(),
                onToggle = {
                    if (status == ConnectionStatus.Connected) VrcWebSocket.disconnect()
                    else VrcWebSocket.connect()
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 10.dp)
            )

            // ═══ 中间：可展开工具面板（手风琴，默认收起）═══
            Column(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(top = 10.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                val disconnected = status != ConnectionStatus.Connected
                ExpandableGlassSection(
                    icon = { LightningGlyph(MaterialTheme.colorScheme.primary, Modifier.fillMaxSize()) },
                    title = "快捷控制",
                    subtitle = "媒体键 · 音量 · 常用应用",
                    expanded = openPanel == RemotePanel.QUICK,
                    onToggle = { openPanel = if (openPanel == RemotePanel.QUICK) null else RemotePanel.QUICK },
                    badge = if (disconnected) "未连接" else null,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    QuickControlGrid(
                        onAction = { VrcWebSocket.sendControl(it) },
                        enabled = status == ConnectionStatus.Connected,
                        showHeader = false,
                        modifier = Modifier.fillMaxWidth()
                    )
                }

                ExpandableGlassSection(
                    icon = { MouseGlyph(MaterialTheme.colorScheme.primary, Modifier.fillMaxSize()) },
                    title = "鼠标控制",
                    subtitle = "触摸板 · 左键 / 确定 / 右键",
                    expanded = openPanel == RemotePanel.MOUSE,
                    onToggle = { openPanel = if (openPanel == RemotePanel.MOUSE) null else RemotePanel.MOUSE },
                    badge = if (disconnected) "未连接" else null,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    MouseControlPanel(
                        connected = status == ConnectionStatus.Connected,
                        onMouseMove = { dx, dy -> VrcWebSocket.sendMouseMove(dx, dy) },
                        onLeftClick = { VrcWebSocket.sendMouseButton("left", "click") },
                        onRightClick = { VrcWebSocket.sendMouseButton("right", "click") },
                        onEnter = { VrcWebSocket.sendEnter() },
                        showHeader = false,
                        modifier = Modifier.fillMaxWidth()
                    )
                }

                // 两面板都收起时，显示轻提示
                if (openPanel == null) {
                    Text(
                        text = "轻点上方面板开启遥控功能",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 24.dp),
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center
                    )
                }
            }

            // ═══ 底部：Siri 式语音按钮（最下方）═══
            SiriMicButton(
                onStart = startMic,
                onStop = { AudioStreamer.stop() },
                active = recording,
                level = micLevel,
                modifier = Modifier.size(76.dp)
            )
            Text(
                text = "按住说话，松开发送",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 6.dp, bottom = 4.dp)
            )
        }

        // Siri 式语音交互覆盖层（最上层）
        SiriVoiceOverlay(
            visible = overlayVisible,
            phase = voicePhase,
            title = voiceTitle,
            subtitle = voiceSubtitle,
            level = micLevel,
            onClose = closeVoiceOverlay,
            onMicStart = startMic,
            onMicStop = { AudioStreamer.stop() },
            onAutoDismiss = { VrcWebSocket.resetVoiceState() },
            pcInputFocus = pcInputFocus,
            inputMode = inputMode,
            onModeChange = VrcWebSocket::setInputMode,
            onQuickCommand = VrcWebSocket::sendControl,
            quickCommandsEnabled = status == ConnectionStatus.Connected,
            partial = voicePartial,
            modifier = Modifier.fillMaxSize()
        )
    }
}

@Preview(showBackground = true)
@Composable
fun RemoteControlPreview() {
    VoiceRemoteControlTheme {
        RemoteControlScreen()
    }
}

@Preview(showBackground = true, uiMode = android.content.res.Configuration.UI_MODE_NIGHT_YES)
@Composable
fun RemoteControlDarkPreview() {
    VoiceRemoteControlTheme {
        RemoteControlScreen()
    }
}
