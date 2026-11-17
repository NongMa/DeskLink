package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

private data class QuickAction(val label: String, val action: String)

private val quickActions = listOf(
    listOf(
        QuickAction("🔉 音量 −", "volume_down"),
        QuickAction("🔇 静音", "volume_mute"),
        QuickAction("🔊 音量 +", "volume_up"),
    ),
    listOf(
        QuickAction("⏮", "prev_track"),
        QuickAction("⏯", "play_pause"),
        QuickAction("⏭", "next_track"),
    ),
    listOf(
        QuickAction("📝 记事本", "open_notepad"),
        QuickAction("🧮 计算器", "open_calc"),
        QuickAction("🌐 浏览器", "open_browser"),
    ),
)

/**
 * 单个玻璃胶囊按钮（快捷控制专用）
 */
@Composable
internal fun LiquidPillButton(
    label: String,
    onClick: () -> Unit,
    enabled: Boolean,
    modifier: Modifier = Modifier
) {
    val isDark = isAppInDarkTheme()
    val pillBg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )

    val backdrop = rememberLayerBackdrop {
        drawRect(pillBg)
        drawContent()
    }

    val interactionSource = remember { MutableInteractionSource() }

    Box(
        modifier = modifier
            .height(56.dp)
            .alpha(if (enabled) 1f else 0.4f)
            .drawBackdrop(
                backdrop = backdrop,
                shape = { RoundedCornerShape(18.dp) },
                effects = {
                    vibrancy()
                    blur(8f.dp.toPx())
                    lens(8f.dp.toPx(), 12f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(pillBg.copy(alpha = 0.65f))
                }
            )
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                enabled = enabled
            ) { onClick() },
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 4.dp)
        )
    }
}

/**
 * 快捷控制面板：媒体键 + 常用应用启动
 * enabled=false 时按钮置灰（未连接 PC）
 */
@Composable
fun QuickControlGrid(
    onAction: (String) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    showHeader: Boolean = true
) {
    val isDark = isAppInDarkTheme()
    val panelBg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )

    val backdrop = rememberLayerBackdrop {
        drawRect(panelBg)
        drawContent()
    }

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
                .fillMaxSize()
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
                        text = "快捷控制",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    if (!enabled) {
                        Text(
                            text = "未连接",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
            quickActions.forEach { row ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    row.forEach { item ->
                        LiquidPillButton(
                            label = item.label,
                            onClick = { onAction(item.action) },
                            enabled = enabled,
                            modifier = Modifier.weight(1f)
                        )
                    }
                }
            }
        }
    }
}
