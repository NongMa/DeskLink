package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.net.ConnectionStatus
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

/**
 * 连接状态条（紧凑版）：状态圆点 + 文案两行 + 连接/断开按钮
 */
@Composable
fun ConnectionStatusBar(
    status: ConnectionStatus,
    address: String,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier
) {
    val isDark = isAppInDarkTheme()
    val bg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )
    val dotColor = when (status) {
        ConnectionStatus.Connected -> Color(0xFF30D158)
        ConnectionStatus.Connecting -> Color(0xFFFFD60A)
        ConnectionStatus.Error -> Color(0xFFFF453A)
        ConnectionStatus.Disconnected -> Color(0xFF8E8E93)
    }
    val title = when (status) {
        ConnectionStatus.Connected -> "已连接到电脑"
        ConnectionStatus.Connecting -> "正在连接…"
        ConnectionStatus.Error -> "连接失败"
        ConnectionStatus.Disconnected -> "未连接"
    }
    val pillLabel = when (status) {
        ConnectionStatus.Connected -> "断开"
        ConnectionStatus.Connecting -> "连接中"
        ConnectionStatus.Error -> "重试"
        ConnectionStatus.Disconnected -> "连接"
    }

    val backdrop = rememberLayerBackdrop {
        drawRect(bg)
        drawContent()
    }
    Box(
        modifier = modifier
            .drawBackdrop(
                backdrop = backdrop,
                shape = { RoundedCornerShape(18.dp) },
                effects = {
                    vibrancy()
                    blur(8f.dp.toPx())
                    lens(10f.dp.toPx(), 16f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(bg.copy(alpha = 0.65f))
                }
            )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            // 状态圆点
            Box(
                modifier = Modifier
                    .size(12.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(dotColor)
            )

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1
                )
                Text(
                    text = address,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1
                )
            }

            LiquidPillButton(
                label = pillLabel,
                onClick = onToggle,
                enabled = status != ConnectionStatus.Connecting,
                modifier = Modifier.width(84.dp)
            )
        }
    }
}
