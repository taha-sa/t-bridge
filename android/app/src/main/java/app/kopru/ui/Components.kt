package app.kopru.ui

import androidx.compose.ui.res.stringResource
import app.kopru.R
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uniffi.kopru_core.ConnState

val CardShape = RoundedCornerShape(16.dp)
val BtnShape = RoundedCornerShape(12.dp)

/** Kart: hafif dikey gradyan, ince çizgi; `gold` ise altın gradyan kenar. */
@Composable
fun TbCard(
    modifier: Modifier = Modifier,
    gold: Boolean = false,
    padding: PaddingValues = PaddingValues(18.dp),
    onClick: (() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val c = tb
    val border = if (gold) BorderStroke(1.dp, c.goldEdge) else BorderStroke(1.dp, c.line)
    Column(
        modifier
            .fillMaxWidth()
            .clip(CardShape)
            .background(c.cardBrush)
            .border(border, CardShape)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(padding),
        content = content,
    )
}

/** Bölüm başlığı: büyük harf, aralıklı, soluk. */
@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier, color: Color = tb.muted) {
    Text(text.uppercase(), modifier, style = MaterialTheme.typography.labelSmall, color = color)
}

@Composable
private fun pressScale(src: MutableInteractionSource): Float {
    val pressed by src.collectIsPressedAsState()
    val s by animateFloatAsState(if (pressed) 0.96f else 1f, tween(120, easing = TbEasing), label = "press")
    return s
}

/** Birincil eylem: altın gradyan. */
@Composable
fun GoldButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    enabled: Boolean = true,
    height: Dp = 52.dp,
) {
    val c = tb
    val src = remember { MutableInteractionSource() }
    Row(
        modifier
            .scale(pressScale(src))
            .height(height)
            .clip(BtnShape)
            .background(if (enabled) c.goldBrush else Brush.linearGradient(listOf(c.s3, c.s3)))
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick)
            .padding(horizontal = 18.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val fg = if (enabled) c.onGold else c.faint
        if (icon != null) {
            Icon(icon, null, tint = fg, modifier = Modifier.size(20.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(text, color = fg, style = MaterialTheme.typography.labelLarge)
    }
}

/** İkincil eylem: koyu yüzey, ince kenar. `danger` kırmızı. */
@Composable
fun TbButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    enabled: Boolean = true,
    danger: Boolean = false,
    height: Dp = 48.dp,
) {
    val c = tb
    val src = remember { MutableInteractionSource() }
    val fg = when {
        !enabled -> c.faint
        danger -> c.bad
        else -> c.text
    }
    Row(
        modifier
            .scale(pressScale(src))
            .height(height)
            .clip(BtnShape)
            .background(if (danger) c.badSoft else c.s3)
            .border(1.dp, if (danger) Color.Transparent else c.line2, BtnShape)
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(icon, null, tint = if (danger || !enabled) fg else c.goldHi, modifier = Modifier.size(19.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(text, color = fg, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Yuvarlak ikon düğmesi. */
@Composable
fun RoundIcon(icon: ImageVector, desc: String, onClick: () -> Unit, size: Dp = 48.dp, primary: Boolean = false, enabled: Boolean = true) {
    val c = tb
    val src = remember { MutableInteractionSource() }
    Box(
        Modifier
            .scale(pressScale(src))
            .size(size)
            .clip(CircleShape)
            .background(if (primary && enabled) c.goldBrush else Brush.linearGradient(listOf(c.s3, c.s3)))
            .border(1.dp, if (primary) Color.Transparent else c.line2, CircleShape)
            .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, desc, tint = if (!enabled) c.faint else if (primary) c.onGold else c.text, modifier = Modifier.size(size * 0.44f))
    }
}

/** Küçük ikon kutucuğu (liste satırlarında). */
@Composable
fun IconTile(icon: ImageVector, tint: Color = tb.goldHi, bg: Color = tb.goldSoft, border: Color = tb.goldLine, size: Dp = 42.dp) {
    Box(
        Modifier.size(size).clip(RoundedCornerShape(12.dp)).background(bg).border(1.dp, border, RoundedCornerShape(12.dp)),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, null, tint = tint, modifier = Modifier.size(size * 0.48f)) }
}

/** Etiket hapı. */
@Composable
fun Pill(text: String, tone: String = "gold") {
    val c = tb
    val (bg, fg) = when (tone) {
        "violet" -> c.violetSoft to Color(0xFFC4B1FF)
        "ok" -> c.ok.copy(alpha = 0.12f) to c.ok
        "bad" -> c.badSoft to c.bad
        "muted" -> c.s3 to c.muted
        else -> c.goldSoft to c.goldHi
    }
    Text(
        text,
        Modifier.clip(RoundedCornerShape(99.dp)).background(bg).padding(horizontal = 10.dp, vertical = 3.dp),
        color = fg,
        style = MaterialTheme.typography.labelMedium,
    )
}

/** Altın anahtar. */
@Composable
fun TbSwitch(checked: Boolean, onChange: (Boolean) -> Unit) {
    val c = tb
    val x by animateFloatAsState(if (checked) 1f else 0f, tween(150, easing = TbEasing), label = "sw")
    val knob by animateColorAsState(if (checked) Color(0xFFFFF8E6) else Color(0xFFD8D5CC), label = "knob")
    Box(
        Modifier
            .size(48.dp, 28.dp)
            .clip(CircleShape)
            .background(if (checked) c.goldBrush else Brush.linearGradient(listOf(c.s4, c.s4)))
            .clickable { onChange(!checked) }
            .padding(3.dp),
    ) {
        Box(Modifier.offset(x = 20.dp * x).size(22.dp).clip(CircleShape).background(knob))
    }
}

/** İlerleme çubuğu; etkinken üstünde parıltı akar. */
@Composable
fun GoldProgress(progress: Float, active: Boolean, tone: String = "gold", modifier: Modifier = Modifier) {
    val c = tb
    val p by animateFloatAsState(progress.coerceIn(0f, 1f), tween(250, easing = TbEasing), label = "p")
    val sheen = if (active) {
        val t = rememberInfiniteTransition(label = "sheen")
        t.animateFloat(-0.4f, 1.4f, infiniteRepeatable(tween(1400, easing = LinearEasing)), label = "s").value
    } else null
    Canvas(modifier.fillMaxWidth().height(6.dp)) {
        val r = CornerRadius(size.height / 2)
        drawRoundRect(c.s4, cornerRadius = r)
        val w = size.width * p
        if (w <= 0f) return@Canvas
        val brush = when (tone) {
            "ok" -> Brush.linearGradient(listOf(c.ok, c.ok))
            "bad" -> Brush.linearGradient(listOf(c.bad, c.bad))
            "muted" -> Brush.linearGradient(listOf(c.faint, c.faint))
            else -> Brush.horizontalGradient(listOf(Color(0xFFF7CF5A), Color(0xFFD99A0B), Color(0xFFB57A06)), endX = w)
        }
        drawRoundRect(brush, size = Size(w, size.height), cornerRadius = r)
        if (sheen != null) {
            val cx = size.width * sheen
            if (cx in 0f..w) {
                drawRoundRect(
                    Brush.horizontalGradient(listOf(Color.Transparent, Color(0x73FFFFFF), Color.Transparent), startX = cx - 60f, endX = cx + 60f),
                    size = Size(w, size.height),
                    cornerRadius = r,
                )
            }
        }
    }
}

/** Bağlantı küresi: bağlıyken altın nefes, bağlanırken dönen yay, değilken sönük. */
@Composable
fun StatusOrb(state: ConnState, size: Dp = 64.dp) {
    val c = tb
    val t = rememberInfiniteTransition(label = "orb")
    val breathe by t.animateFloat(0.9f, 1.18f, infiniteRepeatable(tween(1600, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "b")
    val spin by t.animateFloat(0f, 360f, infiniteRepeatable(tween(1100, easing = LinearEasing)), label = "r")
    Canvas(Modifier.size(size)) {
        val r = this.size.minDimension / 2
        val center = Offset(r, r)
        when (state) {
            ConnState.CONNECTED -> {
                drawCircle(Brush.radialGradient(listOf(Color(0x5CF5C542), Color.Transparent), center, r * 0.95f * breathe), r * breathe, center)
                drawCircle(c.goldLine, r * 0.92f, center, style = Stroke(1.5.dp.toPx()))
                drawCircle(
                    Brush.radialGradient(listOf(Color(0xFFFFE08A), Color(0xFFE3A92B), Color(0xFF8A5A00)), Offset(r * 0.8f, r * 0.75f), r * 0.55f),
                    r * 0.42f,
                    center,
                )
            }
            ConnState.CONNECTING -> {
                drawCircle(c.line2, r * 0.92f, center, style = Stroke(1.5.dp.toPx()))
                drawArc(
                    c.gold, spin, 100f, false,
                    topLeft = Offset(r * 0.08f, r * 0.08f), size = Size(r * 1.84f, r * 1.84f),
                    style = Stroke(2.dp.toPx(), cap = StrokeCap.Round),
                )
                drawCircle(Brush.radialGradient(listOf(Color(0xFFC9B27A), Color(0xFF7D6A3E)), Offset(r * 0.8f, r * 0.75f), r * 0.55f), r * 0.42f, center)
            }
            ConnState.DISCONNECTED -> {
                drawCircle(Brush.radialGradient(listOf(Color(0x1F8F8C84), Color.Transparent), center, r * 0.9f), r * 0.9f, center)
                drawCircle(c.line2, r * 0.92f, center, style = Stroke(1.5.dp.toPx()))
                drawCircle(Brush.radialGradient(listOf(Color(0xFF6B6962), Color(0xFF2B2A27)), Offset(r * 0.8f, r * 0.75f), r * 0.55f), r * 0.42f, center)
            }
        }
    }
}

/** T-Bridge işareti (assets/logo.svg ile aynı geometri). */
@Composable
fun Logo(size: Dp = 40.dp, framed: Boolean = true) {
    Canvas(Modifier.size(size)) {
        val k = this.size.minDimension / 512f
        val gold = Brush.linearGradient(listOf(Color(0xFFF7CF5A), Color(0xFFD99A0B), Color(0xFF9C6400)), Offset(110 * k, 90 * k), Offset(400 * k, 430 * k))
        if (framed) {
            drawRoundRect(Color(0xFF0D0C0A), cornerRadius = CornerRadius(116 * k))
            drawRoundRect(gold, topLeft = Offset(5 * k, 5 * k), size = Size(502 * k, 502 * k), cornerRadius = CornerRadius(111 * k), alpha = 0.4f, style = Stroke(6 * k))
        }
        val arch = Path().apply {
            moveTo(100 * k, 222 * k)
            quadraticTo(256 * k, 92 * k, 412 * k, 222 * k)
        }
        drawPath(arch, gold, style = Stroke(46 * k, cap = StrokeCap.Round))
        drawLine(gold, Offset(256 * k, 168 * k), Offset(256 * k, 404 * k), 54 * k, StrokeCap.Round)
        drawLine(gold, Offset(120 * k, 226 * k), Offset(120 * k, 266 * k), 22 * k, StrokeCap.Round, alpha = 0.85f)
        drawLine(gold, Offset(392 * k, 226 * k), Offset(392 * k, 266 * k), 22 * k, StrokeCap.Round, alpha = 0.85f)
    }
}

/** Ayarların en altı: Instagram bağlantısı + küçük el yazısı imza. */
@Composable
fun SignatureFooter(onInstagram: () -> Unit) {
    val c = tb
    val path = remember { PathParser().parsePathString(SignaturePath.D).toPath() }
    Column(Modifier.fillMaxWidth().padding(top = 26.dp, bottom = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Row(
            Modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = onInstagram).padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Canvas(Modifier.size(16.dp)) {
                val s = size.minDimension
                val st = Stroke(1.6.dp.toPx())
                drawRoundRect(c.muted, topLeft = Offset(s * 0.08f, s * 0.08f), size = Size(s * 0.84f, s * 0.84f), cornerRadius = CornerRadius(s * 0.26f), style = st)
                drawCircle(c.muted, s * 0.2f, Offset(s / 2, s / 2), style = st)
                drawCircle(c.muted, s * 0.05f, Offset(s * 0.74f, s * 0.26f))
            }
            Spacer(Modifier.width(8.dp))
            Text("@tdesk.studio", color = c.muted, style = MaterialTheme.typography.bodyMedium)
        }
        Spacer(Modifier.height(4.dp))
        Canvas(Modifier.size(width = 18.dp * (SignaturePath.W / SignaturePath.H), height = 18.dp)) {
            val k = size.height / SignaturePath.H
            scale(k, k, pivot = Offset.Zero) { drawPath(path, c.faint.copy(alpha = 0.9f)) }
        }
    }
}

/** İzin/kurulum satırı. */
@Composable
fun CheckRow(title: String, desc: String, ok: Boolean, onClick: () -> Unit) {
    val c = tb
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable(enabled = !ok, onClick = onClick).padding(vertical = 10.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(26.dp).clip(CircleShape)
                .background(if (ok) c.ok.copy(alpha = 0.14f) else c.goldSoft)
                .border(1.dp, if (ok) Color.Transparent else c.goldLine, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Canvas(Modifier.size(12.dp)) {
                val s = size.minDimension
                if (ok) {
                    val p = Path().apply { moveTo(s * 0.12f, s * 0.55f); lineTo(s * 0.42f, s * 0.82f); lineTo(s * 0.9f, s * 0.2f) }
                    drawPath(p, c.ok, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round))
                } else {
                    drawCircle(c.goldHi, s * 0.22f)
                }
            }
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(title, color = c.text, style = MaterialTheme.typography.titleSmall)
            Text(desc, color = c.muted, style = MaterialTheme.typography.bodySmall)
        }
        if (!ok) Text(stringResource(R.string.components_open), color = c.goldHi, style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(start = 8.dp))
    }
}

/** Yatay ince ayraç. */
@Composable
fun Hairline(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(1.dp).background(tb.line))
}

/** Satır içinde dikey ayraç. */
@Composable
fun RowScope.VLine() {
    Box(Modifier.width(1.dp).fillMaxHeight().background(tb.line))
}
