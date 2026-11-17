package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.foundation.clickable
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.AppTheme
import cd.cc.nongma.vrc.ui.theme.AppThemeMode
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

/**
 * 液态玻璃切换按钮（深浅主题，三态循环）
 * 跟随系统(🌓) → 深色(🌙) → 浅色(☀️) → 跟随系统 …
 * 切换全局 AppTheme 状态，整个 MaterialTheme 子树随之重组生效。
 */
@Composable
fun LiquidGlassToggle() {
    val mode = AppTheme.mode.value
    val isDark = isAppInDarkTheme()
    val emoji = when (mode) {
        AppThemeMode.System -> "🌓"
        AppThemeMode.Dark -> "🌙"
        AppThemeMode.Light -> "☀️"
    }

    val color = MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
    val surfaceColor = MaterialTheme.colorScheme.surface.copy(alpha = if (isDark) 0.7f else 0.55f)

    Box(
        modifier = Modifier.size(48.dp),
        contentAlignment = Alignment.Center
    ) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .drawBackdrop(
                    backdrop = rememberLayerBackdrop {
                        drawRect(color)
                        drawContent()
                    },
                    shape = { RoundedCornerShape(24.dp) },
                    effects = {
                        vibrancy()
                        blur(4f.dp.toPx())
                        lens(8f.dp.toPx(), 16f.dp.toPx())
                    },
                    onDrawSurface = {
                        drawRect(surfaceColor)
                    }
                )
                .clickable(onClick = { AppTheme.toggle() }),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = emoji,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface
            )
        }
    }
}
