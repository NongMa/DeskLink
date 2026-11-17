package cd.cc.nongma.vrc.ui.glass

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

/**
 * 面板标题图标（与「扫一扫」同款自绘风格，统一为 primary 色描边/填充）。
 * 取代 emoji，视觉上更规整、随主题变色。
 */

/** 快捷控制：闪电 */
@Composable
fun LightningGlyph(color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier) {
        val s = size.minDimension
        val path = Path().apply {
            moveTo(s * 0.58f, s * 0.04f)
            lineTo(s * 0.20f, s * 0.56f)
            lineTo(s * 0.44f, s * 0.56f)
            lineTo(s * 0.38f, s * 0.96f)
            lineTo(s * 0.80f, s * 0.42f)
            lineTo(s * 0.54f, s * 0.42f)
            close()
        }
        drawPath(path, color = color)
    }
}

/** 鼠标控制：鼠标轮廓 + 滚轮 */
@Composable
fun MouseGlyph(color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier) {
        val s = size.minDimension
        val stroke = s / 9f
        val left = s * 0.22f
        val right = s * 0.78f
        val top = s * 0.06f
        val bottom = s * 0.94f
        val w = right - left
        val radius = w * 0.5f
        // 鼠标外壳（圆角矩形轮廓）
        drawRoundRect(
            color = color,
            topLeft = Offset(left, top),
            size = androidx.compose.ui.geometry.Size(w, bottom - top),
            cornerRadius = androidx.compose.ui.geometry.CornerRadius(radius, radius),
            style = Stroke(width = stroke)
        )
        // 左右键分割线
        drawLine(
            color = color,
            start = Offset(left + stroke / 2, top + (bottom - top) * 0.34f),
            end = Offset(right - stroke / 2, top + (bottom - top) * 0.34f),
            strokeWidth = stroke
        )
        // 滚轮
        drawLine(
            color = color,
            start = Offset(s * 0.5f, top + (bottom - top) * 0.13f),
            end = Offset(s * 0.5f, top + (bottom - top) * 0.25f),
            strokeWidth = stroke
        )
    }
}
