package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import androidx.compose.material3.MaterialTheme
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

/**
 * 全屏液态玻璃背景（覆盖整个屏幕）
 * 自动适配深色/浅色主题（跟随系统或用户手动切换）
 */
@Composable
fun LiquidGlassSurface(modifier: Modifier = Modifier) {
    val isDark = isAppInDarkTheme()

    val bgColor = MaterialTheme.colorScheme.surface.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )

    val backdrop = rememberLayerBackdrop {
        drawRect(bgColor)
        drawContent()
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .layerBackdrop(backdrop)
    ) { }
}
