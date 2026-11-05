package app.kopru.ui

import android.content.Context
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/** T-Bridge renkleri: siyah zemin, altın vurgu, nadiren mor (PC arayüzüyle aynı belirteçler). */
@Immutable
data class TbColors(
    val dark: Boolean,
    val bg: Color,
    val s1: Color,
    val s2: Color,
    val s3: Color,
    val s4: Color,
    val line: Color,
    val line2: Color,
    val text: Color,
    val text2: Color,
    val muted: Color,
    val faint: Color,
    val gold: Color,
    val goldHi: Color,
    val goldLo: Color,
    val goldSoft: Color,
    val goldLine: Color,
    val onGold: Color,
    val violet: Color,
    val violetSoft: Color,
    val ok: Color,
    val bad: Color,
    val badSoft: Color,
) {
    val goldBrush: Brush get() = Brush.linearGradient(listOf(Color(0xFFF7CF5A), Color(0xFFD99A0B), Color(0xFF9C6400)))
    val cardBrush: Brush get() = Brush.verticalGradient(listOf(s2, s1))
    val goldEdge: Brush
        get() = Brush.linearGradient(
            listOf(Color(0x8CF7CF5A), Color(0x1FD99A0B), Color(0x0AFFFFFF), Color(0x669C6400)),
        )
}

val DarkTb = TbColors(
    dark = true,
    bg = Color(0xFF0A0A0B), s1 = Color(0xFF111113), s2 = Color(0xFF17171A), s3 = Color(0xFF1E1E22), s4 = Color(0xFF26262B),
    line = Color(0x0FFFFFFF), line2 = Color(0x1AFFFFFF),
    text = Color(0xFFF2F0EA), text2 = Color(0xFFC9C6BD), muted = Color(0xFF8F8C84), faint = Color(0xFF5D5B56),
    gold = Color(0xFFE3A92B), goldHi = Color(0xFFF5C542), goldLo = Color(0xFFA86F00),
    goldSoft = Color(0x1FE3A92B), goldLine = Color(0x52E3A92B), onGold = Color(0xFF1A1204),
    violet = Color(0xFF8B5CF6), violetSoft = Color(0x298B5CF6),
    ok = Color(0xFF4ADE80), bad = Color(0xFFF87171), badSoft = Color(0x1FF87171),
)

val LightTb = TbColors(
    dark = false,
    bg = Color(0xFFF4F2EC), s1 = Color(0xFFFBFAF6), s2 = Color(0xFFFFFFFF), s3 = Color(0xFFF3F0E8), s4 = Color(0xFFE8E4D9),
    line = Color(0x14141008), line2 = Color(0x24141008),
    text = Color(0xFF17150F), text2 = Color(0xFF3C3830), muted = Color(0xFF77726A), faint = Color(0xFFA7A197),
    gold = Color(0xFFB9800B), goldHi = Color(0xFFD99A0B), goldLo = Color(0xFF8A5A00),
    goldSoft = Color(0x1AB9800B), goldLine = Color(0x59B9800B), onGold = Color(0xFF1A1204),
    violet = Color(0xFF7C3AED), violetSoft = Color(0x1A7C3AED),
    ok = Color(0xFF16A34A), bad = Color(0xFFDC2626), badSoft = Color(0x14DC2626),
)

val LocalTb = staticCompositionLocalOf { DarkTb }

/** Kısayol: `tb.gold` vb. */
val tb: TbColors
    @Composable get() = LocalTb.current

/** Tema tercihi ("dark" | "system"); uygulama genelinde gözlemlenebilir. */
object ThemePref {
    val mode = mutableStateOf("dark")
    private var sp: android.content.SharedPreferences? = null

    fun init(ctx: Context) {
        sp = ctx.getSharedPreferences("ui", Context.MODE_PRIVATE)
        mode.value = sp?.getString("theme", "dark") ?: "dark"
    }

    fun set(v: String) {
        mode.value = v
        sp?.edit()?.putString("theme", v)?.apply()
    }
}

private val TbType = Typography(
    headlineLarge = TextStyle(fontSize = 30.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.4).sp, lineHeight = 36.sp),
    headlineMedium = TextStyle(fontSize = 26.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.3).sp, lineHeight = 32.sp),
    titleLarge = TextStyle(fontSize = 20.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.2).sp, lineHeight = 26.sp),
    titleMedium = TextStyle(fontSize = 16.sp, fontWeight = FontWeight.SemiBold, lineHeight = 22.sp),
    titleSmall = TextStyle(fontSize = 14.sp, fontWeight = FontWeight.SemiBold, lineHeight = 20.sp),
    bodyLarge = TextStyle(fontSize = 16.sp, lineHeight = 23.sp),
    bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontSize = 12.5.sp, lineHeight = 17.sp),
    labelLarge = TextStyle(fontSize = 14.sp, fontWeight = FontWeight.SemiBold, lineHeight = 20.sp),
    labelMedium = TextStyle(fontSize = 12.sp, fontWeight = FontWeight.Medium, lineHeight = 16.sp),
    labelSmall = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 1.2.sp, lineHeight = 14.sp),
)

@Composable
fun TBridgeTheme(content: @Composable () -> Unit) {
    val dark = when (ThemePref.mode.value) {
        "system" -> isSystemInDarkTheme()
        else -> true
    }
    val c = if (dark) DarkTb else LightTb
    val scheme = if (dark) {
        darkColorScheme(
            primary = c.gold, onPrimary = c.onGold, primaryContainer = c.goldSoft, onPrimaryContainer = c.goldHi,
            secondary = c.goldHi, onSecondary = c.onGold, tertiary = c.violet,
            background = c.bg, onBackground = c.text, surface = c.s1, onSurface = c.text,
            surfaceVariant = c.s3, onSurfaceVariant = c.muted, surfaceContainer = c.s2, surfaceContainerHigh = c.s3,
            surfaceContainerHighest = c.s4, surfaceContainerLow = c.s1, surfaceContainerLowest = c.bg,
            outline = c.line2, outlineVariant = c.line, error = c.bad, onError = Color.White,
        )
    } else {
        lightColorScheme(
            primary = c.gold, onPrimary = Color.White, primaryContainer = c.goldSoft, onPrimaryContainer = c.goldLo,
            secondary = c.goldHi, tertiary = c.violet,
            background = c.bg, onBackground = c.text, surface = c.s1, onSurface = c.text,
            surfaceVariant = c.s3, onSurfaceVariant = c.muted, surfaceContainer = c.s2, surfaceContainerHigh = c.s3,
            surfaceContainerHighest = c.s4, outline = c.line2, outlineVariant = c.line, error = c.bad,
        )
    }
    CompositionLocalProvider(LocalTb provides c) {
        MaterialTheme(colorScheme = scheme, typography = TbType, content = content)
    }
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
