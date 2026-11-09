package app.kopru.ui

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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import kotlin.math.roundToInt

private fun clickAt(nx: Float, ny: Float, monitor: Int, button: MouseButton) {
    val n = Core.node ?: return
    n.sendScreenPointer(nx, ny, monitor.toUInt())
    n.sendPc(PcCommand.Click(button, false))
}

/**
 * PC ekranı: SurfaceView (MediaCodec çıktısı) + hareketler + otomatik gizlenen üst çubuk.
 * Dokun = sol tık (mutlak konum) · iki parmakla dokun = sağ tık · iki parmak kaydır = kaydırma · çimdik = yakınlaştır ·
 * yakınken tek parmak = kaydır(pan) · basılı tut + sürükle = sürükle-bırak.
 */
@Composable
fun ScreenViewer(
    ui: ScreenUi,
    fullscreen: Boolean,
    onToggleFullscreen: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = tb
    val view = LocalView.current
    var scale by remember { mutableStateOf(1f) }
    var pan by remember { mutableStateOf(Offset.Zero) }
    var box by remember { mutableStateOf(IntSize.Zero) }
    var bump by remember { mutableIntStateOf(0) }
    var barVisible by remember { mutableStateOf(true) }
    var kb by remember { mutableStateOf(false) }
    var monMenu by remember { mutableStateOf(false) }

    DisposableEffect(Unit) {
        view.keepScreenOn = true // izlerken ekran kapanmasın
        onDispose { view.keepScreenOn = false }
    }
    LaunchedEffect(bump, kb, monMenu) {
        barVisible = true
        delay(3000)
        if (!kb && !monMenu) barVisible = false
    }

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
            .pointerInput(ar, monitor) {
                val slop = viewConfiguration.touchSlop
                awaitEachGesture {
                    val first = awaitFirstDown(requireUnconsumed = false)
                    bump++
                    val t0 = SystemClock.uptimeMillis()
                    var moved = 0f
                    var maxPointers = 1
                    var dragging = false
                    var scrollRem = 0f
                    var hRem = 0f
                    var lastSend = 0L
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
                                    if (now - lastSend >= 8) {
                                        lastSend = now
                                        val n = toNorm(p.position)
                                        Core.node?.sendScreenPointer(n.x, n.y, monitor.toUInt())
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
                        if (dragging) Core.node?.sendPc(PcCommand.Button(MouseButton.LEFT, false))
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
                ui.error ?: if (ui.viewing) "Bağlanıyor… (bilgisayarda onay gerekebilir)" else "",
                Modifier.padding(24.dp),
                color = if (ui.error != null) Color(0xFFF87171) else c.muted,
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
                    "Bilgisayarda güvenlik ekranı açık (UAC / kilit) — görüntü geçici olarak durdu",
                    color = c.goldHi,
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text("Ekran kapanınca görüntü kendiliğinden devam eder.", color = c.muted, textAlign = TextAlign.Center, style = MaterialTheme.typography.labelSmall)
            }
        }

        // Üst çubuk (3 sn sonra gizlenir, dokununca geri gelir)
        AnimatedVisibility(
            barVisible,
            Modifier.align(Alignment.TopCenter),
            enter = fadeIn(), exit = fadeOut(),
        ) {
            val shape = RoundedCornerShape(14.dp)
            Column(Modifier.then(if (fullscreen) Modifier.statusBarsPadding() else Modifier).padding(8.dp)) {
                Row(
                    Modifier.clip(shape).background(Color(0xCC111113)).padding(horizontal = 6.dp, vertical = 2.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    if (ui.monitors.size > 1) {
                        Box {
                            TextButton({ monMenu = true }) { Text("Monitör ${monitor + 1} ▾", color = c.goldHi) }
                            DropdownMenu(monMenu, { monMenu = false }) {
                                ui.monitors.forEach { m ->
                                    DropdownMenuItem(
                                        { Text("${m.index.toInt() + 1}: ${m.width}×${m.height}${if (m.primary) " · ana" else ""}") },
                                        { monMenu = false; Screen.setMonitor(m.index.toInt()) },
                                    )
                                }
                            }
                        }
                    }
                    ScreenPreset.entries.forEach { p ->
                        TextButton({ Screen.setPreset(p); bump++ }) {
                            Text(p.label, color = if (ui.preset == p) c.goldHi else c.muted, style = MaterialTheme.typography.labelLarge)
                        }
                    }
                    Text(
                        if (ui.fps > 0) "${ui.fps} fps · ${ui.latencyMs} ms" else "…",
                        color = c.muted,
                        style = MaterialTheme.typography.labelSmall,
                        modifier = Modifier.padding(horizontal = 6.dp),
                    )
                    TextButton({ kb = !kb }) { Text("⌨", modifier = Modifier.semantics { contentDescription = "Klavye" }, color = if (kb) c.goldHi else c.muted) }
                    TextButton({ scale = 1f; pan = Offset.Zero; onToggleFullscreen() }) { Text(if (fullscreen) "⤡" else "⛶", modifier = Modifier.semantics { contentDescription = if (fullscreen) "Tam ekrandan çık" else "Tam ekran" }, color = c.text) }
                    TextButton({ onClose() }) { Text("✕", modifier = Modifier.semantics { contentDescription = "Kapat" }, color = c.text) }
                }
            }
        }

        // Klavye paneli (tam ekranda da kullanılabilir)
        if (kb) {
            Column(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().background(Color(0xE6111113)).padding(8.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                LiveKeyboard()
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    KeyChips()
                }
            }
        }
    }
}
