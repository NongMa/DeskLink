package cd.cc.nongma.vrc.ui.theme

import androidx.compose.runtime.mutableStateOf

enum class AppThemeMode { System, Light, Dark }

/** 当前实际生效的是否深色主题（跟随系统或用户强制） */
@androidx.compose.runtime.Composable
fun isAppInDarkTheme(): Boolean = when (AppTheme.mode.value) {
    AppThemeMode.System -> androidx.compose.foundation.isSystemInDarkTheme()
    AppThemeMode.Light -> false
    AppThemeMode.Dark -> true
}

/**
 * 全局主题模式状态。
 * System = 跟随系统自动切换（需求：自动检测系统深浅）；
 * Light / Dark = 用户手动强制切换（需求：用户侧切换）。
 * 用 Compose 状态持有，切换时整个 MaterialTheme 子树自动重组。
 */
object AppTheme {
    val mode = mutableStateOf(AppThemeMode.System)

    fun toggle() {
        mode.value = when (mode.value) {
            AppThemeMode.System -> AppThemeMode.Dark
            AppThemeMode.Dark -> AppThemeMode.Light
            AppThemeMode.Light -> AppThemeMode.System
        }
    }
}
