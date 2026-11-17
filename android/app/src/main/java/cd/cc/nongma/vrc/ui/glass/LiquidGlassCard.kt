package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
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
 * 信息卡片（设备状态、识别结果等）
 * 深色模式下背景更暗，浅色模式更亮
 */
@Composable
fun LiquidGlassCard(
    title: String,
    status: String,
    modifier: Modifier = Modifier.fillMaxWidth()
) {
    val isDark = isAppInDarkTheme()
    val cardBg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )

    val backdrop = rememberLayerBackdrop {
        drawRect(cardBg)
        drawContent()
    }

    Box(
        modifier = modifier
            .padding(horizontal = 12.dp, vertical = 4.dp)
            .drawBackdrop(
                backdrop = backdrop,
                shape = { RoundedCornerShape(20.dp) },
                effects = {
                    vibrancy()
                    blur(8f.dp.toPx())
                    lens(10f.dp.toPx(), 16f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(cardBg.copy(alpha = 0.65f))
                }
            )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 10.dp, horizontal = 12.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.Start
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = status,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}