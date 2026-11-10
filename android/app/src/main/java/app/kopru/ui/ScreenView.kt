package app.kopru.ui

import androidx.annotation.StringRes
import app.kopru.R
import androidx.compose.ui.res.stringResource
import android.os.SystemClock
import android.view.HapticFeedbackConstants
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import app.kopru.Core
import app.kopru.Screen
import app.kopru.ScreenPreset
import app.kopru.ScreenUi
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.kopru_core.MouseButton
import uniffi.kopru_core.PcCommand
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt

/** Kendi dokunuşunu TÜKETEN küçük yüzey (uzun bas / dokun): alttaki PC-denetim hareket algılayıcısına (tık/sürükle) sızmaz. */
private fun Modifier.ownGestures(onLong: (() -> Unit)? = null, onTap: (() -> Unit)? = null): Modifier = pointerInput(Unit) {
    awaitEachGesture {
        val down = awaitFirstDown()
        down.consume()
        val up = withTimeoutOrNull(500) { waitForUpOrCancellation() }
        if (up == null) {
            onLong?.invoke()
            waitForUpOrCancellation()?.consume()
        } else {
            up.consume()
            onTap?.invoke()
        }
    }
}

private fun clickAt(nx: Float, ny: Float, monitor: Int, button: MouseButton) {
    val n = Core.node ?: return
    n.sendScreenPointer(nx, ny, monitor.toUInt())
    n.sendPc(PcCommand.Click(button, false))
    KeyPad.afterKey() // Ctrl+tık gibi: "sonraki" değiştiriciler tıktan sonra bırakılır
}

/** Ekran kontrol modu: DOKUNMA = dokunduğun noktaya tıkla (mutlak); TOUCHPAD = imleci göreli sür (video üstünde). */
enum class ControlMode(@StringRes val label: Int) { TAP(R.string.scr_mode_tap), PAD(R.string.scr_mode_pad) }

/**
 * Touchpad modu hareketleri (video üstünde, göreli imleç): tek parmak sür = imleç (ivmeli), dokun = sol tık,
 * iki parmakla dokun = sağ tık, iki parmak sürükle = kaydırma, basılı tut + sürükle = sürükle-bırak.
 */
private suspend fun PointerInputScope.padGestures(view: android.view.View, sens: Float, onTouch: () -> Unit) {
    val slop = viewConfiguration.touchSlop
    awaitEachGesture {
        val first = awaitFirstDown(requireUnconsumed = true)
        onTouch()
        val t0 = SystemClock.uptimeMillis()
        var moved = 0f
        var maxPointers = 1
        var dragging = false
        var remX = 0f
        var remY = 0f
        var scrollRem = 0f
        var hRem = 0f
        var lastT = t0
        first.consume()
        try {
            while (true) {
                val wait = if (!dragging && maxPointers == 1 && moved < slop) (450 - (SystemClock.uptimeMillis() - t0)).coerceAtLeast(1) else Long.MAX_VALUE
                val ev = withTimeoutOrNull(wait) { awaitPointerEvent() }
                if (ev == null) {
                    dragging = true
                    view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                    Core.node?.sendPc(PcCommand.Button(MouseButton.LEFT, true))
                    continue
                }
                val pressed = ev.changes.filter { it.pressed }
                maxPointers = maxOf(maxPointers, pressed.size)
                if (pressed.isEmpty()) break
                val now = SystemClock.uptimeMillis()
                val dt = (now - lastT).coerceAtLeast(1)
                lastT = now
                if (pressed.size == 1 && maxPointers == 1) {
                    val p = pressed[0]
                    val d = p.position - p.previousPosition
                    moved += d.getDistance()
                    if (moved > slop || dragging) {
                        val speed = d.getDistance() / dt
                        val f = sens * (1f + min(speed, 3f) * 0.6f)
                        val fx = d.x * f + remX
                        val fy = d.y * f + remY
                        val ix = fx.toInt()
                        val iy = fy.toInt()
                        remX = fx - ix
                        remY = fy - iy
                        if (ix != 0 || iy != 0) Core.node?.sendPointer(ix, iy)
                    }
                } else if (pressed.size >= 2) {
                    var ax = 0f
                    var ay = 0f
                    pressed.forEach { val d = it.position - it.previousPosition; ax += d.x; ay += d.y }
                    ax /= pressed.size
                    ay /= pressed.size
                    moved += abs(ax) + abs(ay)
                    scrollRem += -ay * 3f
                    hRem += -ax * 3f
                    val sy = scrollRem.roundToInt()
                    val sx = hRem.roundToInt()
                    if (abs(sy) >= 8 || abs(sx) >= 8) {
                        Core.node?.sendPc(PcCommand.Scroll(if (abs(sx) >= 8) sx else 0, if (abs(sy) >= 8) sy else 0))
                        if (abs(sy) >= 8) scrollRem -= sy
                        if (abs(sx) >= 8) hRem -= sx
                    }
                }
                ev.changes.forEach { it.consume() }
            }
        } finally {
            if (dragging) {
                Core.node?.sendPc(PcCommand.Button(MouseButton.LEFT, false))
                KeyPad.afterKey()
            }
        }
        val dur = SystemClock.uptimeMillis() - t0
        if (!dragging && moved < slop * 2 && dur < 300) {
            Core.node?.sendPc(PcCommand.Click(if (maxPointers >= 2) MouseButton.RIGHT else MouseButton.LEFT, false))
            KeyPad.afterKey()
        }
    }
}

/**
 * PC ekranı: SurfaceView (MediaCodec çıktısı) + hareketler + otomatik gizlenen küçük durum şeridi (monitör, mod, fps, kapat).
 * DOKUNMA modu: dokun = sol tık (mutlak konum) · iki parmakla dokun = sağ tık · iki parmak kaydır = kaydırma · çimdik =
 * yakınlaştır · yakınken tek parmak = kaydır(pan) · basılı tut + sürükle = sürükle-bırak. TOUCHPAD modu: [padGestures].
 * Görüntü alanının boyutunu PC'ye bildirmek çağıranın işidir (klavye açılınca görüntü küçülür ama yeniden kodlanmaz).
 */
@Composable
fun ScreenViewer(
    ui: ScreenUi,
    mode: ControlMode,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = tb
    val view = LocalView.current
    val sens = remember { view.context.getSharedPreferences("remote", android.content.Context.MODE_PRIVATE).getFloat("sens", 1.6f) }
    var scale by remember { mutableStateOf(1f) }
    var pan by remember { mutableStateOf(Offset.Zero) }
    var box by remember { mutableStateOf(IntSize.Zero) }
    var bump by remember { mutableIntStateOf(0) }
    var barVisible by remember { mutableStateOf(true) }
    var monMenu by remember { mutableStateOf(false) }
    var diagOn by remember { mutableStateOf(false) }
    val diag by Screen.diag.collectAsState()
    // Sürükleme imleci en çok ekran yenileme hızında gönderilir (120 Hz'de ~8 ms); fazlası ağı boş yere doldurur.
    val minGap = remember(view) { (1000f / (view.display?.refreshRate ?: 60f)).toLong().coerceIn(4L, 17L) }

    DisposableEffect(Unit) {
        view.keepScreenOn = true // izlerken ekran kapanmasın
        onDispose { view.keepScreenOn = false }
    }
    // Yakınlaştırma düzeyi PC'ye bildirilir: yakınlaştırınca daha yüksek çözünürlük istenir (metin keskin kalır).
    LaunchedEffect(scale) { Screen.setZoom(scale) }
    LaunchedEffect(bump, monMenu) {
        barVisible = true
        delay(3500)
        if (!monMenu) barVisible = false
    }
    // Touchpad modunda yakınlaştırma yok.
    LaunchedEffect(mode) { if (mode == ControlMode.PAD) { scale = 1f; pan = Offset.Zero } }

    val ar = if (ui.videoW > 0 && ui.videoH > 0) ui.videoW.toFloat() / ui.videoH else 16f / 9f
    val monitor = ui.current

    // Video dikdörtgeni (kutu içinde ortalı, en-boy korunur) ve ekran noktası -> 0..1 eşlemesi.
    fun toNorm(p: Offset): Offset {
        val bw = box.width.toFloat()
        val bh = box.height.toFloat()
        if (bw <= 0f || bh <= 0f) return Offset.Zero
        val vw: Float
        val vh: Float
        if (bw / bh > ar) { vh = bh; vw = bh * ar } else { vw = bw; vh = bw / ar }
        val cx = bw / 2f
        val cy = bh / 2f
        val qx = (p.x - cx - pan.x) / scale + cx
        val qy = (p.y - cy - pan.y) / scale + cy
        val nx = ((qx - (bw - vw) / 2f) / vw).coerceIn(0f, 1f)
        val ny = ((qy - (bh - vh) / 2f) / vh).coerceIn(0f, 1f)
        return Offset(nx, ny)
    }

    fun clampPan(p: Offset): Offset {
        val bw = box.width.toFloat()
        val bh = box.height.toFloat()
        val mx = (scale - 1f) * bw / 2f
        val my = (scale - 1f) * bh / 2f
        return Offset(p.x.coerceIn(-mx, mx), p.y.coerceIn(-my, my))
    }

    Box(
        modifier
            .background(Color.Black)
            .onSizeChanged { box = it }
            .pointerInput(ar, monitor, mode, sens) {
                if (mode == ControlMode.PAD) {
                    padGestures(view, sens) { bump++ }
                    return@pointerInput
                }
                val slop = viewConfiguration.touchSlop
                awaitEachGesture {
                    val first = awaitFirstDown(requireUnconsumed = true)
                    bump++
                    val t0 = SystemClock.uptimeMillis()
                    var moved = 0f
                    var maxPointers = 1
                    var dragging = false
                    var scrollRem = 0f
                    var hRem = 0f
                    var lastSend = 0L
                    var lastDrag: Offset? = null
                    first.consume()
                    try {
                        while (true) {
                            val wait = if (!dragging && maxPointers == 1 && moved < slop) (450 - (SystemClock.uptimeMillis() - t0)).coerceAtLeast(1) else Long.MAX_VALUE
                            val ev = withTimeoutOrNull(wait) { awaitPointerEvent() }
                            if (ev == null) {
                                // Basılı tut: sürükle-bırak başlat (imleç o noktaya, sol tuş basılı).
                                dragging = true
                                view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                                val n = toNorm(first.position)
                                Core.node?.sendScreenPointer(n.x, n.y, monitor.toUInt())
                                Core.node?.sendPc(PcCommand.Button(MouseButton.LEFT, true))
                                continue
                            }
                            val pressed = ev.changes.filter { it.pressed }
                            maxPointers = maxOf(maxPointers, pressed.size)
                            if (pressed.isEmpty()) break
                            if (pressed.size == 1 && maxPointers == 1) {
                                val p = pressed[0]
                                val d = p.position - p.previousPosition
                                moved += d.getDistance()
                                if (dragging) {
                                    val now = SystemClock.uptimeMillis()
                                    val n = toNorm(p.position)
                                    lastDrag = n
                                    if (now - lastSend >= minGap) {
                                        lastSend = now
                                        // Datagram: sıra beklemez, eskiyen konumlar kaybolabilir (son konum aşağıda güvenilir gider).
                                        Core.node?.sendScreenPointerFast(n.x, n.y, monitor.toUInt())
                                    }
                                } else if (scale > 1.01f && moved > slop) {
                                    pan = clampPan(pan + d) // yakınken tek parmak: kaydır (pan)
                                }
                            } else if (pressed.size >= 2) {
                                val a = pressed[0]
                                val b = pressed[1]
                                val dist = hypot(a.position.x - b.position.x, a.position.y - b.position.y)
                                val pdist = hypot(a.previousPosition.x - b.previousPosition.x, a.previousPosition.y - b.previousPosition.y)
                                val centroidD = ((a.position - a.previousPosition) + (b.position - b.previousPosition)) / 2f
                                moved += abs(centroidD.x) + abs(centroidD.y)
                                if (pdist > 0f && abs(dist - pdist) > 1.5f) {
                                    // Çimdik: yakınlaştır (+ merkez kaydırması)
                                    scale = (scale * dist / pdist).coerceIn(1f, 5f)
                                    pan = clampPan(pan + centroidD)
                                    moved += 100f // çimdik tık sayılmaz
                                } else {
                                    // İki parmak sürükle: kaydırma (doğal yön, Touchpad ile aynı)
                                    scrollRem += -centroidD.y * 3f
                                    hRem += -centroidD.x * 3f
                                    val sy = scrollRem.roundToInt()
                                    val sx = hRem.roundToInt()
                                    if (abs(sy) >= 8 || abs(sx) >= 8) {
                                        Core.node?.sendPc(PcCommand.Scroll(if (abs(sx) >= 8) sx else 0, if (abs(sy) >= 8) sy else 0))
                                        if (abs(sy) >= 8) scrollRem -= sy
                                        if (abs(sx) >= 8) hRem -= sx
                                    }
                                }
                            }
                            ev.changes.forEach { it.consume() }
                        }
                    } finally {
                        // İptal/kopma/istisna dahil: sol tuş PC'de basılı kalmasın.
                        if (dragging) {
                            lastDrag?.let { Core.node?.sendScreenPointer(it.x, it.y, monitor.toUInt()) } // kesin son konum
                            Core.node?.sendPc(PcCommand.Button(MouseButton.LEFT, false))
                        }
                    }
                    val dur = SystemClock.uptimeMillis() - t0
                    if (!dragging && moved < slop * 2 && dur < 300) {
                        val n = toNorm(first.position)
                        clickAt(n.x, n.y, monitor, if (maxPointers >= 2) MouseButton.RIGHT else MouseButton.LEFT)
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        AndroidView(
            factory = { ctx ->
                SurfaceView(ctx).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(h: SurfaceHolder) = Screen.attach(h.surface)
                        override fun surfaceChanged(h: SurfaceHolder, f: Int, w: Int, hh: Int) {}
                        override fun surfaceDestroyed(h: SurfaceHolder) = Screen.detach()
                    })
                }
            },
            modifier = Modifier
                .aspectRatio(ar, matchHeightConstraintsFirst = box.height > 0 && box.width.toFloat() / box.height > ar)
                .fillMaxSize()
                .graphicsLayer(scaleX = scale, scaleY = scale, translationX = pan.x, translationY = pan.y),
        )

        if (ui.fps == 0) {
            Text(
                if (ui.errorRes != 0) stringResource(ui.errorRes) else ui.error ?: if (ui.viewing) stringResource(R.string.scr_connecting) else "",
                Modifier.padding(24.dp),
                color = if (ui.error != null || ui.errorRes != 0) Color(0xFFF87171) else c.muted,
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        // PC güvenli masaüstünde (UAC / kilit ekranı): son kare donuk kalır; bilgi kartı üstte, kare silik.
        AnimatedVisibility(ui.secure, Modifier.align(Alignment.Center), enter = fadeIn(), exit = fadeOut()) {
            Column(
                Modifier
                    .padding(24.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(Color(0xF0111113))
                    .border(1.dp, c.goldHi.copy(alpha = 0.32f), RoundedCornerShape(12.dp))
                    .padding(horizontal = 20.dp, vertical = 16.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text("🔒", style = MaterialTheme.typography.headlineSmall)
                Text(
                    stringResource(R.string.scr_secure_title),
                    color = c.goldHi,
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(stringResource(R.string.scr_secure_resume), color = c.muted, textAlign = TextAlign.Center, style = MaterialTheme.typography.labelSmall)
            }
        }

        // Durum şeridi (3,5 sn sonra gizlenir, dokununca geri gelir): monitör · mod · fps/gecikme (uzun bas: teşhis) · kapat
        AnimatedVisibility(
            barVisible,
            Modifier.align(Alignment.TopCenter),
            enter = fadeIn(), exit = fadeOut(),
        ) {
            val shape = RoundedCornerShape(12.dp)
            val stopDesc = stringResource(R.string.scr_stop_sharing)
            Row(
                Modifier.statusBarsPadding().padding(8.dp).clip(shape).background(Color(0xCC111113)).padding(horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                if (ui.monitors.size > 1) {
                    Box {
                        TextButton({ monMenu = true }, Modifier.height(48.dp)) { Text(stringResource(R.string.scr_monitor, monitor + 1), color = c.goldHi) }
                        DropdownMenu(monMenu, { monMenu = false }) {
                            ui.monitors.forEach { m ->
                                DropdownMenuItem(
                                    { Text(stringResource(if (m.primary) R.string.scr_monitor_item_primary else R.string.scr_monitor_item, m.index.toInt() + 1, m.width, m.height)) },
                                    { monMenu = false; Screen.setMonitor(m.index.toInt()) },
                                )
                            }
                        }
                    }
                }
                Text(stringResource(mode.label), color = c.goldHi, style = MaterialTheme.typography.labelMedium, modifier = Modifier.padding(horizontal = 8.dp))
                // Uzun bas: gizli teşhis katmanı (yakalama/kodlama/ağ/çözme/uçtan uca ölçümler).
                Text(
                    if (ui.fps > 0) "${ui.fps} fps · ${ui.latencyMs} ms" else "…",
                    color = Color(0xFFD6D3CB),
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier
                        .padding(horizontal = 6.dp)
                        .ownGestures(onLong = { diagOn = !diagOn; bump++ }),
                )
                TextButton({ onClose() }, Modifier.height(48.dp)) { Text("✕", modifier = Modifier.semantics { contentDescription = stopDesc }, color = Color.White) }
            }
        }

        // Gizli teşhis katmanı: fps göstergesine uzun basınca açılır/kapanır (dokununca kapanır).
        if (diagOn) {
            Column(
                Modifier
                    .align(Alignment.BottomStart)
                    .padding(8.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(Color(0xCC000000))
                    .ownGestures(onTap = { diagOn = false })
                    .padding(horizontal = 8.dp, vertical = 6.dp),
            ) {
                Text(stringResource(R.string.scr_diag_title), color = c.goldHi, style = MaterialTheme.typography.labelSmall)
                if (diag.lines.isEmpty()) Text(stringResource(R.string.scr_diag_waiting), color = c.muted, style = MaterialTheme.typography.labelSmall)
                diag.lines.forEach { Text(it, color = Color.White, style = MaterialTheme.typography.labelSmall) }
            }
        }
    }
}
