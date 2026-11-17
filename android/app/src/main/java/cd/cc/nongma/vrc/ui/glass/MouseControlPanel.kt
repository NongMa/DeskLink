package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme
import kotlin.math.roundToInt

/**
 * 鼠标控制面板：
 * - 大触摸板：手指拖动 → 相对移动电脑鼠标；轻触（几乎不移动的点击）→ 左键单击
 * - 底部按钮：左键 / 确定（回车键）/ 右键
 */
@Composable
fun MouseControlPanel(
    connected: Boolean,
    onMouseMove: (dx: Int, dy: Int) -> Unit,
    onLeftClick: () -> Unit,
    onRightClick: () -> Unit,
    onEnter: () -> Unit,
    modifier: Modifier = Modifier,
    showHeader: Boolean = true
) {
    val isDark = isAppInDarkTheme()
    val panelBg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )
    val padBg = MaterialTheme.colorScheme.surface.copy(
        alpha = if (isDark) 0.35f else 0.5f
    )

    val backdrop = rememberLayerBackdrop {
        drawRect(panelBg)
        drawContent()
    }
    val padBackdrop = rememberLayerBackdrop {
        drawRect(padBg)
        drawContent()
    }

    val sensitivity = 2.2f
    // 供 pointerInput 内部读取最新连接状态（pointerInput 块不随重组重建）
    val connectedState = rememberUpdatedState(connected)

    Box(
        modifier = modifier
            .padding(horizontal = 12.dp)
            .drawBackdrop(
                backdrop = backdrop,
                shape = { RoundedCornerShape(20.dp) },
                effects = {
                    vibrancy()
                    blur(8f.dp.toPx())
                    lens(10f.dp.toPx(), 16f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(panelBg.copy(alpha = 0.65f))
                }
            )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            if (showHeader) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "鼠标控制",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    if (!connected) {
                        Text(
                            text = "未连接",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }

            // 触摸板
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(170.dp)
                    .alpha(if (connected) 1f else 0.4f)
                    .drawBackdrop(
                        backdrop = padBackdrop,
                        shape = { RoundedCornerShape(16.dp) },
                        effects = {
                            vibrancy()
                            blur(6f.dp.toPx())
                            lens(6f.dp.toPx(), 10f.dp.toPx())
                        },
                        onDrawSurface = {
                            drawRect(padBg.copy(alpha = 0.6f))
                        }
                    )
                    .pointerInput(Unit) {
                        awaitPointerEventScope {
                            while (true) {
                                val down = awaitFirstDown(requireUnconsumed = false)
                                if (!connectedState.value) continue
                                var startX = down.position.x
                                var startY = down.position.y
                                var lastX = startX
                                var lastY = startY
                                var moved = false
                                while (true) {
                                    val event = awaitPointerEvent()
                                    val change = event.changes.firstOrNull() ?: continue
                                    if (!change.pressed) {
                                        // 轻触（几乎未移动）→ 左键单击
                                        if (!moved) onLeftClick()
                                        break
                                    }
                                    val dx = change.position.x - lastX
                                    val dy = change.position.y - lastY
                                    lastX = change.position.x
                                    lastY = change.position.y
                                    val totalDx = change.position.x - startX
                                    val totalDy = change.position.y - startY
                                    if (kotlin.math.abs(totalDx) > 24f || kotlin.math.abs(totalDy) > 24f) {
                                        moved = true
                                    }
                                    val mx = (dx * sensitivity).roundToInt().coerceIn(-120, 120)
                                    val my = (dy * sensitivity).roundToInt().coerceIn(-120, 120)
                                    if (mx != 0 || my != 0) {
                                        onMouseMove(mx, my)
                                    }
                                    change.consume()
                                }
                            }
                        }
                    },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = "拖动移动鼠标 · 轻触左键",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }

            // 按键行
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                LiquidPillButton(
                    label = "🖱 左键",
                    onClick = onLeftClick,
                    enabled = connected,
                    modifier = Modifier.weight(1f)
                )
                LiquidPillButton(
                    label = "⏎ 确定",
                    onClick = onEnter,
                    enabled = connected,
                    modifier = Modifier.weight(1f)
                )
                LiquidPillButton(
                    label = "右键",
                    onClick = onRightClick,
                    enabled = connected,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}
