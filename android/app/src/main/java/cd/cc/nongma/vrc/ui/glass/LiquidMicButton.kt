package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

/**
 * 主麦克风按钮
 * 按住说话，松开停止并发送结束帧
 * 深色模式下按钮颜色更暗，浅色模式更亮（跟随全局主题状态）
 */
@Composable
fun LiquidMicButton(
    onStart: () -> Unit,
    onStop: () -> Unit,
    modifier: Modifier = Modifier.size(96.dp)
) {
    val isDark = isAppInDarkTheme()
    val primaryColor = if (isDark) {
        MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
    } else {
        MaterialTheme.colorScheme.primary.copy(alpha = 0.2f)
    }
    val surfaceColor = MaterialTheme.colorScheme.surface.copy(alpha = if (isDark) 0.6f else 0.55f)

    val backdrop = rememberLayerBackdrop {
        drawRect(primaryColor)
        drawContent()
    }

    Box(
        modifier = modifier
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        awaitFirstDown()
                        onStart()
                        var lifted = false
                        while (!lifted) {
                            val event = awaitPointerEvent()
                            lifted = event.changes.all { change -> !change.pressed }
                        }
                        onStop()
                    }
                }
            }
            .drawBackdrop(
                backdrop = backdrop,
                shape = { RoundedCornerShape(48.dp) },
                effects = {
                    vibrancy()
                    blur(2f.dp.toPx())
                    lens(12f.dp.toPx(), 24f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(surfaceColor)
                    drawRect(primaryColor.copy(alpha = 0.3f))
                }
            ),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = "🎤",
            style = MaterialTheme.typography.displayMedium,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}
