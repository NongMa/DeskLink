package cd.cc.nongma.vrc.ui.glass

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.animation.core.animateFloatAsState
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme

/**
 * 可展开的玻璃工具面板：
 * - 默认收起：只显示一行「图标 + 标题 + 副标题 + 箭头」
 * - 点击后展开/收起，带竖向展开动画
 * 面板内容由调用方提供（快捷控制网格 / 鼠标控制面板等）
 *
 * icon 由调用方提供（一般为自绘 Canvas glyph，统一走 primary 色）。
 */
@Composable
fun ExpandableGlassSection(
    icon: @Composable () -> Unit,
    title: String,
    subtitle: String,
    expanded: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
    badge: String? = null,
    content: @Composable () -> Unit
) {
    val isDark = isAppInDarkTheme()
    val headerBg = MaterialTheme.colorScheme.surfaceContainerHighest.copy(
        alpha = if (isDark) 0.7f else 0.55f
    )
    val iconBg = MaterialTheme.colorScheme.primary.copy(alpha = if (isDark) 0.35f else 0.18f)
    val backdrop = rememberLayerBackdrop {
        drawRect(headerBg)
        drawContent()
    }
    val interactionSource = remember { MutableInteractionSource() }

    Column(modifier = modifier) {
        // 头部（玻璃胶囊，可点击）
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .drawBackdrop(
                    backdrop = backdrop,
                    shape = { RoundedCornerShape(18.dp) },
                    effects = {
                        vibrancy()
                        blur(8f.dp.toPx())
                        lens(10f.dp.toPx(), 16f.dp.toPx())
                    },
                    onDrawSurface = {
                        drawRect(headerBg.copy(alpha = 0.65f))
                    }
                )
                .clickable(
                    interactionSource = interactionSource,
                    indication = null,
                    onClick = onToggle
                )
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(60.dp)
                    .padding(horizontal = 14.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // 图标底圈
                Box(
                    modifier = Modifier
                        .size(36.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .background(iconBg),
                    contentAlignment = Alignment.Center
                ) {
                    Box(modifier = Modifier.size(22.dp), contentAlignment = Alignment.Center) {
                        icon()
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = title,
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSurface,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1
                    )
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1
                    )
                }
                if (badge != null) {
                    Text(
                        text = badge,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier
                            .padding(end = 8.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .background(MaterialTheme.colorScheme.error.copy(alpha = 0.12f))
                            .padding(horizontal = 8.dp, vertical = 3.dp)
                    )
                }
                // 液态玻璃下拉箭头：默认收起时指向右，展开时旋转朝下
                GlassChevron(expanded = expanded)
            }
        }

        // 展开内容
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(expandFrom = Alignment.Top) + fadeIn(),
            exit = shrinkVertically(shrinkTowards = Alignment.Top) + fadeOut()
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
            ) {
                content()
            }
        }
    }
}

/**
 * 液态玻璃下拉箭头：与主题切换按钮同款毛玻璃圆（34dp），内绘制 Chevron。
 * 收起时向右，展开时旋转朝下，带过渡动画。
 */
@Composable
private fun GlassChevron(expanded: Boolean, modifier: Modifier = Modifier) {
    val isDark = isAppInDarkTheme()
    val color = MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
    val chevronColor = MaterialTheme.colorScheme.primary
    val surfaceColor = MaterialTheme.colorScheme.surface.copy(alpha = if (isDark) 0.7f else 0.55f)
    val chevronAngle by animateFloatAsState(
        targetValue = if (expanded) 0f else -90f,
        animationSpec = androidx.compose.animation.core.tween(220),
        label = "chevron"
    )
    Box(
        modifier = modifier
            .size(34.dp)
            .rotate(chevronAngle)
            .drawBackdrop(
                backdrop = rememberLayerBackdrop {
                    drawRect(color)
                    drawContent()
                },
                shape = { RoundedCornerShape(17.dp) },
                effects = {
                    vibrancy()
                    blur(4f.dp.toPx())
                    lens(8f.dp.toPx(), 16f.dp.toPx())
                },
                onDrawSurface = {
                    drawRect(surfaceColor)
                }
            ),
        contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.size(17.dp)) {
            val s = size.minDimension
            val stroke = s / 6.2f
            val path = Path().apply {
                moveTo(s * 0.24f, s * 0.36f)
                lineTo(s * 0.50f, s * 0.62f)
                lineTo(s * 0.76f, s * 0.36f)
            }
            drawPath(
                path = path,
                color = chevronColor,
                style = Stroke(
                    width = stroke,
                    cap = StrokeCap.Round,
                    join = StrokeJoin.Round
                )
            )
        }
    }
}
