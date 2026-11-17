package cd.cc.nongma.vrc.ui.glass

import androidx.compose.animation.core.*
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * Siri 式麦克风按钮：纯渐变声光球体，无图标。
 * 待机时缓慢呼吸；按住说话时光斑加速旋转、涟漪向外扩散、球体随电平膨胀。
 * 按下时整体缩小回弹，松开立即还原。
 *
 * 渲染分层：
 *  ① 外层（满画布）：聆听涟漪 + 呼吸光晕
 *  ② 内层（圆形裁剪）：半透明蓝紫球壳（外缘渐隐，无黑边）+ 五色光斑（Additive 混合，重叠处发光不发灰）+ 高光
 */
@Composable
fun SiriMicButton(
    onStart: () -> Unit,
    onStop: () -> Unit,
    active: Boolean,
    level: Float,
    modifier: Modifier = Modifier.size(118.dp)
) {
    var pressed by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(
        targetValue = if (pressed) 0.88f else 1f,
        animationSpec = tween(180, easing = FastOutSlowInEasing),
        label = "pressScale"
    )

    val transition = rememberInfiniteTransition(label = "siriMic")
    val rot by transition.animateFloat(
        0f, 360f,
        infiniteRepeatable(tween(if (active) 2600 else 6500, easing = LinearEasing)),
        label = "rot"
    )
    val rotReverse by transition.animateFloat(
        360f, 0f,
        infiniteRepeatable(tween(9800, easing = LinearEasing)),
        label = "rotReverse"
    )
    val breathe by transition.animateFloat(
        0f, 1f,
        infiniteRepeatable(
            tween(if (active) 520 else 2100, easing = FastOutSlowInEasing),
            RepeatMode.Reverse
        ),
        label = "breathe"
    )
    val ripple by transition.animateFloat(
        0f, 1f,
        infiniteRepeatable(tween(1700, easing = LinearEasing)),
        label = "ripple"
    )
    val levelSm by animateFloatAsState(level, tween(110), label = "level")

    Box(
        modifier = modifier
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        awaitFirstDown()
                        pressed = true
                        onStart()
                        var lifted = false
                        while (!lifted) {
                            val event = awaitPointerEvent()
                            lifted = event.changes.all { change -> !change.pressed }
                        }
                        pressed = false
                        onStop()
                    }
                }
            },
        contentAlignment = Alignment.Center
    ) {
        // ── ① 光环层：聆听涟漪 + 呼吸光晕（可溢出球体）──
        Canvas(modifier = Modifier.fillMaxSize()) {
            val s = size.minDimension
            val c = center
            val baseR = s * 0.44f * (1f + 0.10f * breathe + 0.18f * levelSm * if (active) 1f else 0f) * scale
            val glowR = s * 0.50f

            if (active) {
                for (k in 0..1) {
                    val t = (ripple + k * 0.5f) % 1f
                    drawCircle(
                        color = Color(0xFF8A7BFF).copy(alpha = (1f - t) * 0.35f * (0.4f + levelSm)),
                        radius = baseR * (1f + t * 1.05f),
                        center = c,
                        style = androidx.compose.ui.graphics.drawscope.Stroke(width = 3.dp.toPx())
                    )
                }
            }

            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(
                        Color(0xFF8A7BFF).copy(alpha = 0.26f + 0.22f * breathe + 0.30f * levelSm),
                        Color(0xFF5FE3FF).copy(alpha = 0.10f),
                        Color.Transparent
                    ),
                    radius = glowR
                ),
                radius = glowR,
                center = c
            )
        }

        // ── ② 球体层：圆形裁剪，内部光斑用 Additive 混合 ──
        Box(
            modifier = Modifier
                .fillMaxSize()
                .clip(CircleShape)
        ) {
            Canvas(modifier = Modifier.fillMaxSize()) {
                val s = size.minDimension
                val c = center
                // 裁剪后画布更小，按同比例换算球半径（视觉上保持与光环层一致的比例）
                val baseR = s * 0.44f * (1f + 0.10f * breathe + 0.18f * levelSm * if (active) 1f else 0f) * scale

                // 球壳（径向渐变，中心亮边缘蓝紫，外缘逐渐透明化避免硬黑边）
                drawCircle(
                    brush = Brush.radialGradient(
                        0f to Color(0xFF4A557F),
                        0.4f to Color(0xFF333D63),
                        0.72f to Color(0xFF252C49).copy(alpha = 0.9f),
                        0.88f to Color(0xFF1F253B).copy(alpha = 0.55f),
                        1f to Color.Transparent
                    ),
                    radius = baseR,
                    center = c
                )

                // 两朵缓慢反向旋转的极光雾（大半径、低 alpha、Additive）
                val mists = listOf(
                    Triple(Color(0xFF3A2D8F), 0.0, 0.52f),
                    Triple(Color(0xFF0E3D54), 180.0, 0.52f),
                )
                for ((color, offsetDeg, ratio) in mists) {
                    val ang = Math.toRadians(rotReverse + offsetDeg)
                    val mx = c.x + (kotlin.math.cos(ang) * baseR * ratio).toFloat()
                    val my = c.y + (kotlin.math.sin(ang) * baseR * ratio * 0.8).toFloat()
                    drawCircle(
                        brush = Brush.radialGradient(
                            colors = listOf(
                                color.copy(alpha = 0.55f),
                                color.copy(alpha = 0.18f),
                                Color.Transparent
                            ),
                            radius = baseR * 0.95f
                        ),
                        radius = baseR * 0.95f,
                        center = Offset(mx, my),
                        blendMode = BlendMode.Plus
                    )
                }

                // 五色流光光斑（纯饱和色 + Additive：重叠区域增亮，不再发灰；
                // 轨道放宽、alpha 压低，避免中心过曝成白团）
                val blobs = listOf(
                    Triple(Color(0xFF00D5FF), 0.0, 0.52f),   // 青
                    Triple(Color(0xFF6E4DFF), 72.0, 0.46f),  // 蓝紫
                    Triple(Color(0xFFFF3D9A), 144.0, 0.60f), // 粉
                    Triple(Color(0xFFFFB02E), 216.0, 0.47f), // 橙
                    Triple(Color(0xFF00E896), 288.0, 0.56f), // 绿
                )
                val blobAlpha = if (active) 0.92f else 0.78f
                for ((color, offsetDeg, orbitRatio) in blobs) {
                    val ang = Math.toRadians(rot + offsetDeg)
                    val orbit = baseR * orbitRatio * (1.0f + levelSm * 0.35f + 0.10f * breathe)
                    val bx = c.x + (kotlin.math.cos(ang) * orbit).toFloat()
                    val by = c.y + (kotlin.math.sin(ang) * orbit * 0.78).toFloat()
                    val blobR = baseR * (0.44f + 0.16f * levelSm + 0.05f * breathe)
                    drawCircle(
                        brush = Brush.radialGradient(
                            colors = listOf(
                                color.copy(alpha = blobAlpha),
                                color.copy(alpha = blobAlpha * 0.42f),
                                Color.Transparent
                            ),
                            radius = blobR
                        ),
                        radius = blobR,
                        center = Offset(bx, by),
                        blendMode = BlendMode.Plus
                    )
                }

                // 顶部镜面高光（普通混合，压低亮度避免盖住色彩）
                drawCircle(
                    brush = Brush.radialGradient(
                        colors = listOf(Color.White.copy(alpha = 0.22f), Color.Transparent),
                        radius = baseR * 0.38f
                    ),
                    radius = baseR * 0.38f,
                    center = Offset(c.x - baseR * 0.27f, c.y - baseR * 0.32f)
                )
            }
        }
    }
}
