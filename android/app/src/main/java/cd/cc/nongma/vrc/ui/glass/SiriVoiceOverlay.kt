package cd.cc.nongma.vrc.ui.glass

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import cd.cc.nongma.vrc.net.VoiceInputMode
import cd.cc.nongma.vrc.net.VoicePhase
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme
import kotlinx.coroutines.delay

/** 覆盖层配色：随 App 深/浅主题切换 */
private data class OverlayPalette(
    val scrim: Color,
    val topLabel: Color,
    val closeBg: Color,
    val closeIcon: Color,
    val hint: Color,
    val primary: Color,
    val primaryDim: Color,
    val accent: Color,
    val accentError: Color,
)

@Composable
private fun overlayPalette(): OverlayPalette =
    if (isAppInDarkTheme()) {
        OverlayPalette(
            scrim = Color(0xFF060714),
            topLabel = Color.White.copy(alpha = 0.75f),
            closeBg = Color.White.copy(alpha = 0.10f),
            closeIcon = Color.White.copy(alpha = 0.9f),
            hint = Color.White.copy(alpha = 0.55f),
            primary = Color.White,
            primaryDim = Color.White.copy(alpha = 0.8f),
            accent = Color(0xFF5FE3FF),
            accentError = Color(0xFFFF8A80),
        )
    } else {
        OverlayPalette(
            scrim = Color(0xFFF1F3F9),
            topLabel = Color(0xFF1B1F2A).copy(alpha = 0.70f),
            closeBg = Color(0xFF1B1F2A).copy(alpha = 0.08f),
            closeIcon = Color(0xFF1B1F2A).copy(alpha = 0.85f),
            hint = Color(0xFF1B1F2A).copy(alpha = 0.60f),
            primary = Color(0xFF141824),
            primaryDim = Color(0xFF141824).copy(alpha = 0.85f),
            accent = Color(0xFF0E8FB8),
            accentError = Color(0xFFD32F2F),
        )
    }

/**
 * Siri 式全屏语音交互覆盖层：
 * 全面屏沉浸背景 → 流光球体（随麦克风电平起伏）→ 实时声波 → 大标题/结果胶囊 → 底部按住说话。
 * 背景延伸到状态栏/导航栏之后（全面屏），内容通过 safeDrawingPadding 避开刘海与系统栏。
 * 松开麦克风后显示识别结果，Result/Error 阶段约 6.5s 后自动收起。
 */
@Composable
fun SiriVoiceOverlay(
    visible: Boolean,
    phase: VoicePhase,
    title: String,
    subtitle: String,
    level: Float,
    onClose: () -> Unit,
    onMicStart: () -> Unit,
    onMicStop: () -> Unit,
    onAutoDismiss: () -> Unit,
    pcInputFocus: Boolean = false,
    inputMode: VoiceInputMode = VoiceInputMode.Text,
    onModeChange: (VoiceInputMode) -> Unit = {},
    onQuickCommand: (String) -> Unit = {},
    quickCommandsEnabled: Boolean = false,
    partial: String = "",
    modifier: Modifier = Modifier,
) {
    val phaseState = rememberUpdatedState(phase)
    val palette = overlayPalette()

    // 结果/错误展示一段时间后自动回到 Idle
    LaunchedEffect(phase) {
        if (phase == VoicePhase.Result || phase == VoicePhase.Error) {
            delay(6500)
            if (phaseState.value == phase) onAutoDismiss()
        }
    }

    AnimatedVisibility(
        visible = visible,
        modifier = modifier.fillMaxSize(),
        enter = fadeIn(tween(260)),
        exit = fadeOut(tween(220)),
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(palette.scrim)
        ) {
            // 外层 Column 吃掉刘海/系统栏 inset，内部布局不再写死 top 间距
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .safeDrawingPadding()
            ) {
                // 顶部行：语音标签 + 关闭按钮
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 14.dp, start = 22.dp, end = 18.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "语  音",
                        color = palette.topLabel,
                        fontSize = 14.sp,
                        letterSpacing = 4.sp
                    )
                    Spacer(Modifier.weight(1f))
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(CircleShape)
                            .background(palette.closeBg)
                            .clickable { onClose() },
                        contentAlignment = Alignment.Center
                    ) {
                        Text("✕", color = palette.closeIcon, fontSize = 17.sp)
                    }
                }

                // 中段：球体 + 声波 + 文案
                Column(
                    modifier = Modifier.weight(1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center
                ) {
                    SiriOrb(level = level, phase = phase, modifier = Modifier.size(190.dp))
                    Spacer(Modifier.height(26.dp))
                    WaveBars(
                        level = level,
                        modifier = Modifier
                            .fillMaxWidth(0.82f)
                            .height(46.dp)
                    )
                    Spacer(Modifier.height(24.dp))
                    StatusText(
                        phase = phase,
                        title = title,
                        subtitle = subtitle,
                        modifier = Modifier.fillMaxWidth(),
                        partial = partial,
                        hint = if (pcInputFocus && inputMode == VoiceInputMode.Text)
                            "说出要输入的文字内容"
                        else
                            "说出要执行的电脑指令，例如：打开微信 / 音量加"
                    )
                }

                // 底部：按住说话（再次录音）
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    // PC 输入框聚焦时显示双模式切换（转文字输入 / 指令识别）
                    AnimatedVisibility(
                        visible = pcInputFocus,
                        enter = fadeIn(tween(200)),
                        exit = fadeOut(tween(160))
                    ) {
                        InputModeSegmented(
                            mode = inputMode,
                            onModeChange = onModeChange,
                            palette = palette
                        )
                    }
                    // 常见指令快捷方式：点击直接下发 control 指令
                    QuickCommandChips(
                        enabled = quickCommandsEnabled,
                        onQuickCommand = onQuickCommand,
                        palette = palette,
                        modifier = Modifier.fillMaxWidth()
                    )
                    SiriMicButton(
                        onStart = onMicStart,
                        onStop = onMicStop,
                        active = phase == VoicePhase.Listening,
                        level = level,
                        modifier = Modifier.size(88.dp)
                    )
                    Text(
                        text = if (phase == VoicePhase.Listening) "松开结束，发送语音" else "按住说话",
                        color = palette.hint,
                        fontSize = 12.sp
                    )
                }
            }
        }
    }
}

/** 双模式切换：转文字输入 / 指令识别（仅当 PC 输入框聚焦时由上层控制显隐） */
@Composable
private fun InputModeSegmented(
    mode: VoiceInputMode,
    onModeChange: (VoiceInputMode) -> Unit,
    palette: OverlayPalette,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .clip(RoundedCornerShape(22.dp))
            .background(palette.closeBg)
            .padding(4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ModeSegment(
            label = "转文字输入",
            selected = mode == VoiceInputMode.Text,
            palette = palette,
            onClick = { onModeChange(VoiceInputMode.Text) },
        )
        ModeSegment(
            label = "指令识别",
            selected = mode == VoiceInputMode.Command,
            palette = palette,
            onClick = { onModeChange(VoiceInputMode.Command) },
        )
    }
}

@Composable
private fun ModeSegment(
    label: String,
    selected: Boolean,
    palette: OverlayPalette,
    onClick: () -> Unit,
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(18.dp))
            .background(
                if (selected) palette.accent.copy(alpha = 0.30f) else Color.Transparent
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 8.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (selected) palette.accent else palette.hint,
            fontSize = 13.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
}

/** 常见指令快捷入口 */
private data class QuickChip(val label: String, val action: String)

private val quickCommandChips = listOf(
    QuickChip("⏯ 播放/暂停", "play_pause"),
    QuickChip("⏮ 上一曲", "prev_track"),
    QuickChip("⏭ 下一曲", "next_track"),
    QuickChip("🔊 音量+", "volume_up"),
    QuickChip("🔉 音量−", "volume_down"),
    QuickChip("🔇 静音", "volume_mute"),
    QuickChip("⏎ 回车", "enter"),
    QuickChip("🌐 浏览器", "open_browser"),
    QuickChip("📝 记事本", "open_notepad"),
    QuickChip("🧮 计算器", "open_calc"),
)

/** 常见指令快捷方式横滑列表：点击直接下发 control 动作 */
@Composable
private fun QuickCommandChips(
    enabled: Boolean,
    onQuickCommand: (String) -> Unit,
    palette: OverlayPalette,
    modifier: Modifier = Modifier,
) {
    val scrollState = rememberScrollState()
    Row(
        modifier = modifier
            .horizontalScroll(scrollState)
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        quickCommandChips.forEach { chip ->
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(18.dp))
                    .background(palette.closeBg)
                    .clickable(enabled = enabled) { onQuickCommand(chip.action) }
                    .padding(horizontal = 14.dp, vertical = 8.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = chip.label,
                    color = if (enabled) palette.primaryDim else palette.hint,
                    fontSize = 12.sp,
                )
            }
        }
    }
}
