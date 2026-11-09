package app.kopru.ui

import android.content.Context
import android.os.SystemClock
import android.view.HapticFeedbackConstants
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.VolumeDown
import androidx.compose.material.icons.automirrored.outlined.VolumeOff
import androidx.compose.material.icons.automirrored.outlined.VolumeUp
import androidx.compose.material.icons.outlined.ChevronLeft
import androidx.compose.material.icons.outlined.ChevronRight
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.SkipNext
import androidx.compose.material.icons.outlined.SkipPrevious
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.ui.platform.LocalContext as LocalCtx
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.Screen
import android.app.Activity
import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import app.kopru.CameraService
import android.content.pm.ActivityInfo
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.kopru.Core
import app.kopru.UiState
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.MediaAction
import uniffi.kopru_core.MouseButton
import uniffi.kopru_core.PcCommand
import uniffi.kopru_core.PowerAction
import uniffi.kopru_core.SpecialKey
import uniffi.kopru_core.VolumeAction
import kotlin.math.abs
import kotlin.math.min
import kotlin.math.roundToInt

private fun pc(cmd: PcCommand) {
    Core.node?.sendPc(cmd)
}

private fun key(k: SpecialKey) = pc(PcCommand.Key(k))

@Composable
fun RemoteScreen(ui: UiState) {
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("remote", Context.MODE_PRIVATE) }
    var sens by remember { mutableFloatStateOf(prefs.getFloat("sens", 1.6f)) }
    var confirm by remember { mutableStateOf<PowerAction?>(null) }
    val on = ui.conn == ConnState.CONNECTED

    val c = tb
    Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Row(Modifier.padding(top = 4.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Kumanda", Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Pill(if (on) "canlı" else "bağlı değil", if (on) "violet" else "muted")
        }

        val scr by Screen.ui.collectAsStateWithLifecycle()
        var full by rememberSaveable { mutableStateOf(false) }
        ScreenLifecycle()
        ScreenToggle(scr, on)
        if (scr.viewing) {
            val ar = if (scr.videoW > 0 && scr.videoH > 0) scr.videoW.toFloat() / scr.videoH else 16f / 9f
            if (!full) {
                ScreenViewer(
                    scr, false,
                    onToggleFullscreen = { full = true },
                    onClose = { Screen.stopViewing() },
                    modifier = Modifier.fillMaxWidth().aspectRatio(ar).clip(RoundedCornerShape(16.dp)),
                )
            } else {
                FullscreenViewer(scr, onExit = { full = false }, onClose = { full = false; Screen.stopViewing() })
            }
        } else {
            Touchpad(sens)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            TbButton("Sol tık", { pc(PcCommand.Click(MouseButton.LEFT, false)) }, Modifier.weight(1f))
            TbButton("Orta", { pc(PcCommand.Click(MouseButton.MIDDLE, false)) }, Modifier.weight(0.6f))
            TbButton("Sağ tık", { pc(PcCommand.Click(MouseButton.RIGHT, false)) }, Modifier.weight(1f))
        }
        Text(
            "Tek parmak: imleç · dokun: tık · iki parmakla dokun: sağ tık · iki parmak kaydır: kaydırma · basılı tut + sürükle: sürükle-bırak",
            color = c.faint,
            style = MaterialTheme.typography.bodySmall,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Hız", Modifier.width(44.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
            Slider(
                sens,
                { sens = it; prefs.edit().putFloat("sens", it).apply() },
                valueRange = 0.6f..4f,
                modifier = Modifier.weight(1f),
                colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
            )
        }

        CameraSection(on)

        Section("Klavye") {
            LiveKeyboard()
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                KeyChips()
            }
        }

        Section("Medya ve ses") {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
                RoundIcon(Icons.Outlined.SkipPrevious, "Önceki", { pc(PcCommand.Media(MediaAction.PREVIOUS)) })
                Spacer(Modifier.width(16.dp))
                RoundIcon(Icons.Outlined.PlayArrow, "Oynat/Duraklat", { pc(PcCommand.Media(MediaAction.PLAY_PAUSE)) }, size = 62.dp, primary = true)
                Spacer(Modifier.width(16.dp))
                RoundIcon(Icons.Outlined.SkipNext, "Sonraki", { pc(PcCommand.Media(MediaAction.NEXT)) })
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TbButton("Kıs", { pc(PcCommand.Volume(VolumeAction.DOWN)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeDown, height = 44.dp)
                TbButton("Sessiz", { pc(PcCommand.Volume(VolumeAction.MUTE)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeOff, height = 44.dp)
                TbButton("Aç", { pc(PcCommand.Volume(VolumeAction.UP)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeUp, height = 44.dp)
            }
        }

        Section("Sunum") {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TbButton("Geri", { key(SpecialKey.LEFT) }, Modifier.weight(1f), icon = Icons.Outlined.ChevronLeft, height = 64.dp)
                TbButton("İleri", { key(SpecialKey.RIGHT) }, Modifier.weight(1f), icon = Icons.Outlined.ChevronRight, height = 64.dp)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TbButton("Başlat (F5)", { key(SpecialKey.F5) }, Modifier.weight(1f), height = 44.dp)
                TbButton("Bitir (Esc)", { key(SpecialKey.ESCAPE) }, Modifier.weight(1f), height = 44.dp)
            }
        }

        Section("Güç") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TbButton("Kilitle", { pc(PcCommand.Power(PowerAction.LOCK)) }, Modifier.weight(1f), height = 44.dp)
                TbButton("Uyku", { confirm = PowerAction.SLEEP }, Modifier.weight(1f), height = 44.dp)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TbButton("Yeniden başlat", { confirm = PowerAction.RESTART }, Modifier.weight(1f), danger = true, height = 44.dp)
                TbButton("Kapat", { confirm = PowerAction.SHUTDOWN }, Modifier.weight(1f), danger = true, height = 44.dp)
            }
        }
    }

    confirm?.let { a ->
        val name = when (a) {
            PowerAction.SLEEP -> "uyku moduna alınsın"
            PowerAction.RESTART -> "yeniden başlatılsın"
            PowerAction.SHUTDOWN -> "kapatılsın"
            PowerAction.LOCK -> "kilitlensin"
        }
        AlertDialog(
            onDismissRequest = { confirm = null },
            containerColor = c.s2,
            title = { Text("Bilgisayar $name mı?") },
            text = { Text("Kaydedilmemiş işler kaybolabilir.", color = c.text2) },
            confirmButton = { TextButton({ pc(PcCommand.Power(a)); confirm = null }) { Text("Evet", color = c.bad) } },
            dismissButton = { TextButton({ confirm = null }) { Text("Vazgeç", color = c.text2) } },
        )
    }
}

@Composable
internal fun KeyChips() {
    Chip("Esc") { key(SpecialKey.ESCAPE) }
    Chip("Tab") { key(SpecialKey.TAB) }
    Chip("⌫") { key(SpecialKey.BACKSPACE) }
    Chip("Del") { key(SpecialKey.DELETE) }
    Chip("Enter") { key(SpecialKey.ENTER) }
    Chip("←") { key(SpecialKey.LEFT) }
    Chip("↑") { key(SpecialKey.UP) }
    Chip("↓") { key(SpecialKey.DOWN) }
    Chip("→") { key(SpecialKey.RIGHT) }
    Chip("Home") { key(SpecialKey.HOME) }
    Chip("End") { key(SpecialKey.END) }
    Chip("PgUp") { key(SpecialKey.PAGE_UP) }
    Chip("PgDn") { key(SpecialKey.PAGE_DOWN) }
    Chip("Win") { key(SpecialKey.WIN) }
    Chip("Alt+Tab") { key(SpecialKey.ALT_TAB) }
    Chip("Kopyala") { key(SpecialKey.COPY_SHORTCUT) }
    Chip("Yapıştır") { key(SpecialKey.PASTE_SHORTCUT) }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    TbCard {
        SectionTitle(title)
        Spacer(Modifier.height(12.dp))
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) { content() }
    }
}

@Composable
internal fun Chip(label: String, onClick: () -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(10.dp)
    Text(
        label,
        Modifier
            .clip(shape)
            .background(c.s3)
            .border(1.dp, c.line2, shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        color = c.text,
        style = MaterialTheme.typography.labelMedium,
    )
}

/** Yazılan metin anında PC'ye gider: eklenen karakterler metin olarak, silinenler Backspace olarak. */
@Composable
internal fun LiveKeyboard() {
    var text by remember { mutableStateOf("") }
    OutlinedTextField(
        value = text,
        onValueChange = { new ->
            val old = text
            var p = 0
            val max = min(old.length, new.length)
            while (p < max && old[p] == new[p]) p++
            // Vekil çiftin ortasında bölme.
            if (p > 0 && p < old.length && Character.isLowSurrogate(old[p])) p--
            val deleted = old.codePointCount(p, old.length)
            repeat(deleted) { key(SpecialKey.BACKSPACE) }
            val added = new.substring(p)
            if (added.isNotEmpty()) pc(PcCommand.Text(added))
            // Uzun tamponu kısa tut (diff ucuz kalsın).
            text = if (new.length > 200) "" else new
        },
        modifier = Modifier.fillMaxWidth(),
        placeholder = { Text("Buraya yaz, anında bilgisayara gider", color = tb.faint) },
        shape = RoundedCornerShape(14.dp),
        colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = tb.gold, unfocusedBorderColor = tb.line2, cursorColor = tb.goldHi),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
        keyboardActions = KeyboardActions(onSend = { key(SpecialKey.ENTER); text = "" }),
        singleLine = true,
    )
}

/** Touchpad: göreli hareket (ivmeli), dokun/çift parmak tık, iki parmak kaydırma, basılı tut-sürükle. */
@Composable
private fun Touchpad(sens: Float) {
    val view = LocalView.current
    val c = tb
    val shape = RoundedCornerShape(24.dp)
    val scope = rememberCoroutineScope()
    val ripple = remember { Animatable(1f) }
    var rippleAt by remember { mutableStateOf(Offset.Zero) }
    Box(
        Modifier
            .fillMaxWidth()
            .height(300.dp)
            .clip(shape)
            .background(c.cardBrush)
            .border(1.dp, c.goldEdge, shape)
            .pointerInput(sens) {
                val slop = viewConfiguration.touchSlop
                awaitEachGesture {
                    val first = awaitFirstDown(requireUnconsumed = false)
                    val t0 = SystemClock.uptimeMillis()
                    var moved = 0f
                    var maxPointers = 1
                    var dragging = false
                    var remX = 0f
                    var remY = 0f
                    var scrollRem = 0f
                    var hScrollRem = 0f
                    var lastT = t0
                    first.consume()
                    rippleAt = first.position
                    scope.launch {
                        ripple.snapTo(0f)
                        ripple.animateTo(1f, tween(520))
                    }
                    while (true) {
                        val wait = if (!dragging && maxPointers == 1 && moved < slop) (450 - (SystemClock.uptimeMillis() - t0)).coerceAtLeast(1) else Long.MAX_VALUE
                        val ev = withTimeoutOrNull(wait) { awaitPointerEvent() }
                        if (ev == null) {
                            // Basılı tutuldu: sürükleme başlat.
                            dragging = true
                            view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                            pc(PcCommand.Button(MouseButton.LEFT, true))
                            continue
                        }
                        val pressed = ev.changes.filter { it.pressed }
                        maxPointers = maxOf(maxPointers, pressed.size)
                        if (pressed.isEmpty()) break
                        val now = SystemClock.uptimeMillis()
                        val dt = (now - lastT).coerceAtLeast(1)
                        lastT = now
                        if (pressed.size == 1 && maxPointers == 1) {
                            val c = pressed[0]
                            val d: Offset = c.position - c.previousPosition
                            moved += d.getDistance()
                            if (moved > slop || dragging) {
                                // İvme: hızlı hareket daha uzağa gider.
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
                            // Doğal kaydırma (telefondaki gibi): parmak yukarı → aşağıdaki içerik gelir (tekerlek aşağı).
                            // PC tarafı: tekerlek = -dy, yatay tekerlek = dx.
                            scrollRem += -ay * 3f
                            hScrollRem += -ax * 3f
                            val sy = scrollRem.roundToInt()
                            val sx = hScrollRem.roundToInt()
                            if (abs(sy) >= 8 || abs(sx) >= 8) {
                                pc(PcCommand.Scroll(if (abs(sx) >= 8) sx else 0, if (abs(sy) >= 8) sy else 0))
                                if (abs(sy) >= 8) scrollRem -= sy
                                if (abs(sx) >= 8) hScrollRem -= sx
                            }
                        }
                        ev.changes.forEach { it.consume() }
                    }
                    val dur = SystemClock.uptimeMillis() - t0
                    if (dragging) {
                        pc(PcCommand.Button(MouseButton.LEFT, false))
                    } else if (moved < slop * 2 && dur < 300) {
                        pc(PcCommand.Click(if (maxPointers >= 2) MouseButton.RIGHT else MouseButton.LEFT, false))
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        // Dokunulan noktada altın dalga (imleç kanalını etkilemez; yalnızca çizim).
        Canvas(Modifier.fillMaxSize()) {
            val p = ripple.value
            if (p < 1f) {
                drawCircle(
                    Color(0xFFF5C542).copy(alpha = (1f - p) * 0.55f),
                    radius = 18.dp.toPx() + p * 70.dp.toPx(),
                    center = rippleAt,
                    style = Stroke(1.5.dp.toPx()),
                )
            }
        }
        Text("TOUCHPAD", color = c.faint, style = MaterialTheme.typography.labelSmall)
    }
}

/** Uygulama arka plana gidince çözmeyi durdur (pil), dönünce sürdür; sekmeden çıkınca paylaşımı kapat. */
@Composable
private fun ScreenLifecycle() {
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val obs = LifecycleEventObserver { _, e ->
            if (e == Lifecycle.Event.ON_STOP) Screen.pause()
            if (e == Lifecycle.Event.ON_START) Screen.resume()
        }
        owner.lifecycle.addObserver(obs)
        onDispose {
            owner.lifecycle.removeObserver(obs)
            Screen.stopViewing()
        }
    }
}

@Composable
private fun ScreenToggle(scr: app.kopru.ScreenUi, connected: Boolean) {
    val c = tb
    val supported = connected && Screen.supported()
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text("Ekranı göster", color = c.text, style = MaterialTheme.typography.titleSmall)
            Text(
                when {
                    !connected -> "Bağlanınca bilgisayar ekranını burada görürsün"
                    !supported -> "Bilgisayardaki T-Bridge sürümü ekran paylaşımını desteklemiyor"
                    scr.error != null -> scr.error
                    else -> "Bilgisayar ekranı canlı; dokunarak kontrol et"
                },
                color = if (scr.error != null) Color(0xFFF87171) else c.faint,
                style = MaterialTheme.typography.bodySmall,
            )
        }
        Switch(
            checked = scr.viewing,
            onCheckedChange = { if (it) Screen.startViewing() else Screen.stopViewing() },
            enabled = supported,
            colors = SwitchDefaults.colors(checkedTrackColor = c.gold, checkedThumbColor = c.onGold),
        )
    }
}

@Composable
private fun FullscreenViewer(scr: app.kopru.ScreenUi, onExit: () -> Unit, onClose: () -> Unit) {
    val ctx = LocalCtx.current
    DisposableEffect(Unit) {
        val a = ctx as? Activity
        a?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        onDispose { a?.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED }
    }
    Dialog(onDismissRequest = onExit, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val v = LocalView.current
        val w = (v.parent as? DialogWindowProvider)?.window
        SideEffect {
            w?.let {
                WindowCompat.setDecorFitsSystemWindows(it, false)
                WindowInsetsControllerCompat(it, it.decorView).apply {
                    systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                    hide(WindowInsetsCompat.Type.systemBars())
                }
            }
        }
        ScreenViewer(scr, true, onToggleFullscreen = onExit, onClose = onClose, modifier = Modifier.fillMaxSize())
    }
}

/** "Telefonu web kamerası yap": yalnızca PC `camera` yeteneğini duyuruyorsa (Windows 11) kullanılabilir. */
@Composable
private fun CameraSection(connected: Boolean) {
    val c = tb
    val ctx = LocalContext.current
    val cam by CameraService.ui.collectAsStateWithLifecycle()
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        granted = ok
        if (ok) CameraService.start(ctx)
    }
    val supported = connected && CameraService.supported()

    Section("Kamera") {
        if (!supported) {
            // Kilitli durum: Windows 10 veya eski PC sürümü
            Text(
                if (connected) "Bu özellik yalnızca Windows 11 için tasarlanmıştır." else "Bağlanınca telefonu bilgisayarda web kamerası olarak kullanabilirsin.",
                color = c.faint,
                style = MaterialTheme.typography.bodyMedium,
            )
            return@Section
        }
        Text(
            "Telefonunu bilgisayarın web kamerası yap. Görüntü yalnızca bilgisayarına, şifreli bağlantıyla gider. Kamera izni gerekir; yayın sürerken bildirim görünür.",
            color = c.muted,
            style = MaterialTheme.typography.bodySmall,
        )
        if (cam.error != null) Text(cam.error!!, color = Color(0xFFF87171), style = MaterialTheme.typography.bodySmall)
        TbButton(
            if (cam.streaming) "Yayını durdur" else "Yayını başlat",
            {
                if (cam.streaming) CameraService.stop(ctx)
                else if (granted) CameraService.start(ctx)
                else ask.launch(Manifest.permission.CAMERA)
            },
            Modifier.fillMaxWidth(),
        )
        if (cam.streaming) Text("Yayında · ${cam.w}×${cam.h}", color = c.goldHi, style = MaterialTheme.typography.labelLarge)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Chip(if (cam.front) "Ön kamera ✓" else "Ön kamera") { CameraService.setFront(true) }
            Chip(if (!cam.front) "Arka kamera ✓" else "Arka kamera") { CameraService.setFront(false) }
            Chip(if (cam.res == 720) "720p ✓" else "720p") { CameraService.setRes(720) }
            Chip(if (cam.res == 1080) "1080p ✓" else "1080p") { CameraService.setRes(1080) }
            Chip(if (cam.mirror) "Ayna ✓" else "Ayna") { CameraService.setMirror(!cam.mirror) }
            Chip(if (cam.torch) "Fener ✓" else "Fener") { CameraService.setTorch(!cam.torch) }
        }
        if (cam.exposureRange.last > cam.exposureRange.first) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Pozlama", Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
                Slider(
                    cam.exposure.toFloat(),
                    { CameraService.setExposure(it.roundToInt()) },
                    valueRange = cam.exposureRange.first.toFloat()..cam.exposureRange.last.toFloat(),
                    modifier = Modifier.weight(1f),
                    colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
                )
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Yakınlık", Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
            Slider(
                cam.zoom,
                { CameraService.setZoom(it) },
                valueRange = 0f..1f,
                modifier = Modifier.weight(1f),
                colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
            )
        }
        Text("Telefonu dikey ya da yatay tutabilirsin; bilgisayara her zaman dik görüntü gider.", color = c.faint, style = MaterialTheme.typography.bodySmall)
    }
}
