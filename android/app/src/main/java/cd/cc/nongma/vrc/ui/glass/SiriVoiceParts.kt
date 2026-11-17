package cd.cc.nongma.vrc.ui.glass

import androidx.compose.animation.core.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import cd.cc.nongma.vrc.net.VoicePhase
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/** 流光球体：深色球壳 + 五色光斑绕心旋转 + 电平驱动膨胀/高亮 */
@Composable
fun SiriOrb(level: Float, phase: VoicePhase, modifier: Modifier = Modifier) {
    val transition = rememberInfiniteTransition(label = "orb")
    val rot by transition.animateFloat(
        0f, 360f,
        infiniteRepeatable(tween(9000, easing = LinearEasing)),
        label = "rot"
    )
    val pulse by transition.animateFloat(
        0.93f, 1.07f,
        infiniteRepeatable(tween(1100, easing = FastOutSlowInEasing), RepeatMode.Reverse),
        label = "pulse"
    )
    val levelSm by animateFloatAsState(level, tween(120, easing = FastOutSlowInEasing), label = "lvl")
    val processingDim by animateFloatAsState(
        if (phase == VoicePhase.Processing) 0.45f else 0f,
        tween(400),
        label = "dim"
    )

    Canvas(modifier = modifier) {
        val s = size.minDimension
        val c = center
        val baseR = s * 0.30f * pulse * (1f + levelSm * 0.15f)

        // 外层呼吸光晕
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(
                    Color(0xFF8A7BFF).copy(alpha = 0.34f + levelSm * 0.28f),
                    Color(0xFF5FE3FF).copy(alpha = 0.10f),
                    Color.Transparent
                ),
                radius = s * 0.52f
            ),
            radius = s * 0.52f,
            center = c
        )

        // 球体本体（深蓝径向渐变）
        drawCircle(
            brush = Brush.radialGradient(
                0f to Color(0xFF2E3752),
                0.55f to Color(0xFF1B2238),
                1f to Color(0xFF0C1020)
            ),
            radius = baseR,
            center = c
        )

        // 五个流光光斑（椭圆轨道绕心旋转，Additive 混合：重叠增亮不发灰）
        val blobs = listOf(
            Triple(Color(0xFF00D5FF), 0.0, 0.36f),
            Triple(Color(0xFF6E4DFF), 72.0, 0.32f),
            Triple(Color(0xFFFF3D9A), 144.0, 0.42f),
            Triple(Color(0xFFFFB02E), 216.0, 0.34f),
            Triple(Color(0xFF00E896), 288.0, 0.39f),
        )
        for ((color, offsetDeg, orbitRatio) in blobs) {
            val ang = Math.toRadians(rot + offsetDeg)
            val orbit = s * orbitRatio * (0.80f + levelSm * 0.35f)
            val bx = c.x + (cos(ang) * orbit).toFloat()
            val by = c.y + (sin(ang) * orbit * 0.82).toFloat()
            val blobR = baseR * (0.60f + levelSm * 0.32f)
            val alpha = 0.72f - processingDim * 0.40f
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(
                        color.copy(alpha = alpha),
                        color.copy(alpha = alpha * 0.40f),
                        Color.Transparent
                    ),
                    radius = blobR
                ),
                radius = blobR,
                center = Offset(bx, by),
                blendMode = BlendMode.Plus
            )
        }

        // 顶部高光
        drawCircle(
            brush = Brush.radialGradient(
                colors = listOf(Color.White.copy(alpha = 0.30f), Color.Transparent),
                radius = baseR * 0.62f
            ),
            radius = baseR * 0.62f,
            center = Offset(c.x - baseR * 0.30f, c.y - baseR * 0.36f)
        )
    }
}

/** 46 根竖条的声波：中间高两侧低，电平驱动幅度，正弦行波制造流动感 */
@Composable
fun WaveBars(level: Float, modifier: Modifier = Modifier) {
    val transition = rememberInfiniteTransition(label = "wave")
    val wavePhase by transition.animateFloat(
        0f, (2 * PI).toFloat(),
        infiniteRepeatable(tween(1400, easing = LinearEasing)),
        label = "ph"
    )
    val levelSm by animateFloatAsState(level, tween(100, easing = FastOutSlowInEasing), label = "wlvl")
    val staticOffset = remember { 3.7f }
    val barBrush = remember {
        Brush.linearGradient(
            colors = listOf(Color(0xFF5FE3FF), Color(0xFF8A7BFF), Color(0xFFFF6FB7))
        )
    }

    Canvas(modifier = modifier) {
        val w = size.width
        val h = size.height
        val barCount = 46
        val slot = w / barCount
        val barW = slot * 0.46f
        for (i in 0 until barCount) {
            val t = i.toFloat() / (barCount - 1)
            val center01 = 1f - kotlin.math.abs(t - 0.5f) * 2f
            val envelope = sin(PI * center01).toFloat().coerceIn(0f, 1f)
            val wobble = 0.5f + 0.5f * sin(wavePhase * 2f + i * 0.55f + staticOffset)
            val amp = 0.10f + 0.90f * levelSm
            var barH = h * (0.10f + 0.82f * envelope * (0.35f + 0.65f * amp) * (0.55f + 0.45f * wobble))
            barH = barH.coerceIn(h * 0.06f, h)
            val x = slot * i + (slot - barW) / 2f
            val y = (h - barH) / 2f
            drawRoundRect(
                brush = barBrush,
                topLeft = Offset(x, y),
                size = Size(barW, barH),
                cornerRadius = CornerRadius(barW / 2f, barW / 2f),
                alpha = 0.45f + 0.55f * (0.3f + 0.7f * levelSm)
            )
        }
    }
}

/** 阶段文案：聆听中实时转写 / 识别中动画点 / 结果胶囊 / 错误提示（颜色随深浅主题切换） */
@Composable
fun StatusText(
    phase: VoicePhase,
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier,
    hint: String = "说出要执行的电脑指令，例如：打开浏览器",
    partial: String = ""
) {
    val dark = isAppInDarkTheme()
    val primary = if (dark) Color.White else Color(0xFF141824)
    val primaryDim = if (dark) Color.White.copy(alpha = 0.8f) else Color(0xFF141824).copy(alpha = 0.85f)
    val hintColor = if (dark) Color.White.copy(alpha = 0.55f) else Color(0xFF141824).copy(alpha = 0.60f)
    val accent = if (dark) Color(0xFF5FE3FF) else Color(0xFF0E8FB8)
    val error = if (dark) Color(0xFFFF8A80) else Color(0xFFD32F2F)
    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        when (phase) {
            VoicePhase.Listening -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                if (partial.isNotBlank()) {
                    Text(
                        partial,
                        color = primary,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.SemiBold,
                        textAlign = TextAlign.Center,
                        maxLines = 3,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )
                } else {
                    Text(
                        "正在聆听…",
                        color = primary,
                        fontSize = 24.sp,
                        fontWeight = FontWeight.SemiBold
                    )
                }
                Spacer(Modifier.height(8.dp))
                Text(
                    hint,
                    color = hintColor,
                    fontSize = 13.sp,
                    textAlign = TextAlign.Center
                )
            }
            VoicePhase.Processing -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                ProcessingDots()
                Spacer(Modifier.height(10.dp))
                Text(
                    "识别中，请稍候…",
                    color = primaryDim,
                    fontSize = 18.sp
                )
            }
            VoicePhase.Result -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    title.ifBlank { "（未听清）" },
                    color = primary,
                    fontSize = 19.sp,
                    fontWeight = FontWeight.Medium,
                    textAlign = TextAlign.Center,
                    maxLines = 2
                )
                Spacer(Modifier.height(12.dp))
                ResultPill(text = subtitle, accent = accent)
            }
            VoicePhase.Error -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(
                    title.ifBlank { "出错了" },
                    color = error,
                    fontSize = 22.sp,
                    fontWeight = FontWeight.SemiBold
                )
                Spacer(Modifier.height(12.dp))
                ResultPill(text = subtitle, accent = error)
            }
            VoicePhase.Idle -> Unit
        }
    }
}

@Composable
private fun ResultPill(text: String, accent: Color) {
    Box(
        modifier = Modifier
            .background(
                color = accent.copy(alpha = 0.16f),
                shape = RoundedCornerShape(50)
            )
            .padding(horizontal = 18.dp, vertical = 8.dp)
    ) {
        Text(text, color = accent, fontSize = 14.sp, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun ProcessingDots() {
    val transition = rememberInfiniteTransition(label = "dots")
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        repeat(3) { i ->
            val a by transition.animateFloat(
                0.25f, 1f,
                infiniteRepeatable(
                    animation = tween(450, delayMillis = i * 140, easing = FastOutSlowInEasing),
                    repeatMode = RepeatMode.Reverse
                ),
                label = "dot$i"
            )
            Box(
                modifier = Modifier
                    .size(12.dp)
                    .clip(CircleShape)
                    .background(Color(0xFF8A7BFF).copy(alpha = a))
            )
        }
    }
}
