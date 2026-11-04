package app.kopru.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val Blue = Color(0xFF2563EB)

@Composable
fun KopruTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val ctx = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= 31 && dark -> dynamicDarkColorScheme(ctx)
        Build.VERSION.SDK_INT >= 31 -> dynamicLightColorScheme(ctx)
        dark -> darkColorScheme(primary = Color(0xFF7FA6FF), secondary = Color(0xFF7FA6FF))
        else -> lightColorScheme(primary = Blue, secondary = Blue)
    }
    MaterialTheme(colorScheme = scheme, content = content)
}

fun fmtBytes(n: Long): String {
    if (n < 1024) return "$n B"
    val u = arrayOf("KB", "MB", "GB", "TB")
    var v = n / 1024.0
    var i = 0
    while (v >= 1024 && i < u.size - 1) {
        v /= 1024
        i++
    }
    return if (v >= 100) "%.0f %s".format(v, u[i]) else "%.1f %s".format(v, u[i])
}
