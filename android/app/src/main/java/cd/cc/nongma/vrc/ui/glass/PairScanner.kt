package cd.cc.nongma.vrc.ui.glass

import android.Manifest
import android.content.pm.PackageManager
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import cd.cc.nongma.vrc.ui.theme.isAppInDarkTheme
import cd.cc.nongma.vrc.net.VrcWebSocket
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions

/**
 * 应用内扫码配对：返回一个「点击即扫码」的动作。
 * 扫码内容为 vrc://pair?host=..&port=..&token=.. 时自动保存配置并重连。
 * 需要相机运行时权限；没有权限时会先请求权限。
 */
@Composable
fun rememberPairScanner(): () -> Unit {
    val context = LocalContext.current

    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        val contents = result.contents
        val ok = !contents.isNullOrBlank() && VrcWebSocket.applyPairText(contents)
        Toast.makeText(
            context,
            if (ok) "配对成功，正在连接…"
            else "未识别到有效的配对二维码（应为 vrc://pair 链接）",
            Toast.LENGTH_LONG,
        ).show()
    }

    val permLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            scanLauncher.launch(scanOptions())
        } else {
            Toast.makeText(context, "需要相机权限才能扫码配对", Toast.LENGTH_LONG).show()
        }
    }

    return {
        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        if (granted) scanLauncher.launch(scanOptions()) else permLauncher.launch(Manifest.permission.CAMERA)
    }
}

private fun scanOptions() = ScanOptions().apply {
    setDesiredBarcodeFormats(ScanOptions.QR_CODE)
    setPrompt("扫描电脑管理页上的配对二维码")
    setBeepEnabled(false)
    setOrientationLocked(true)
}

/**
 * 扫一扫配对入口（右上角常驻）。
 * 与主题切换按钮保持同款液态玻璃圆：48dp 圆形毛玻璃 + 扫一扫四角括号彩色图标。
 * 点击后在应用内打开全屏扫描器自动完成配对。
 */
@Composable
fun ScanPairFab(modifier: Modifier = Modifier) {
    val onScanPair = rememberPairScanner()
    val isDark = isAppInDarkTheme()
    val color = MaterialTheme.colorScheme.primary.copy(alpha = 0.3f)
    val surfaceColor = MaterialTheme.colorScheme.surface.copy(alpha = if (isDark) 0.7f else 0.55f)
    Box(
        modifier = modifier
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
            .clickable(onClick = onScanPair)
            .semantics { contentDescription = "扫一扫配对" },
        contentAlignment = Alignment.Center,
    ) {
        ScanGlyph(
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.size(24.dp),
        )
    }
}

/** 扫一扫风格图标：四角括号 */
@Composable
private fun ScanGlyph(color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier) {
        val s = size.minDimension
        val stroke = s / 8.5f
        val inset = s * 0.10f
        val arm = s * 0.30f
        val x0 = inset
        val y0 = inset
        val x1 = s - inset
        val y1 = s - inset

        fun drawLineX1(y: Float, xa: Float, xb: Float) =
            drawLine(color, Offset(xa, y), Offset(xb, y), strokeWidth = stroke)
        fun drawLineY1(x: Float, ya: Float, yb: Float) =
            drawLine(color, Offset(x, ya), Offset(x, yb), strokeWidth = stroke)

        // 左上
        drawLineX1(y0, x0, x0 + arm)
        drawLineY1(x0, y0, y0 + arm)
        // 右上
        drawLineX1(y0, x1 - arm, x1)
        drawLineY1(x1, y0, y0 + arm)
        // 左下
        drawLineX1(y1, x0, x0 + arm)
        drawLineY1(x0, y1 - arm, y1)
        // 右下
        drawLineX1(y1, x1 - arm, x1)
        drawLineY1(x1, y1 - arm, y1)
    }
}
