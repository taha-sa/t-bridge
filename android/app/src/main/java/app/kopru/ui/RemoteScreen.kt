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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
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

    Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        if (!on) Text("Bilgisayar bağlı değil.", color = MaterialTheme.colorScheme.error)

        Touchpad(sens)
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            FilledTonalButton({ pc(PcCommand.Click(MouseButton.LEFT, false)) }, Modifier.weight(1f)) { Text("Sol tık") }
            FilledTonalButton({ pc(PcCommand.Click(MouseButton.MIDDLE, false)) }, Modifier.weight(0.6f)) { Text("Orta") }
            FilledTonalButton({ pc(PcCommand.Click(MouseButton.RIGHT, false)) }, Modifier.weight(1f)) { Text("Sağ tık") }
        }
        Text(
            "Tek parmak: imleç · dokun: tık · iki parmakla dokun: sağ tık · iki parmak kaydır: kaydırma · basılı tut + sürükle: sürükle-bırak",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Hız", Modifier.width(48.dp))
            Slider(sens, { sens = it; prefs.edit().putFloat("sens", it).apply() }, valueRange = 0.6f..4f, modifier = Modifier.weight(1f))
        }

        Section("Klavye") {
            LiveKeyboard()
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
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
        }

        Section("Medya ve ses") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalIconButton({ pc(PcCommand.Media(MediaAction.PREVIOUS)) }) { Icon(Icons.Outlined.SkipPrevious, "Önceki") }
                FilledTonalIconButton({ pc(PcCommand.Media(MediaAction.PLAY_PAUSE)) }) { Icon(Icons.Outlined.PlayArrow, "Oynat/Duraklat") }
                FilledTonalIconButton({ pc(PcCommand.Media(MediaAction.NEXT)) }) { Icon(Icons.Outlined.SkipNext, "Sonraki") }
                Spacer(Modifier.width(12.dp))
                FilledTonalIconButton({ pc(PcCommand.Volume(VolumeAction.DOWN)) }) { Icon(Icons.AutoMirrored.Outlined.VolumeDown, "Kıs") }
                FilledTonalIconButton({ pc(PcCommand.Volume(VolumeAction.UP)) }) { Icon(Icons.AutoMirrored.Outlined.VolumeUp, "Aç") }
                FilledTonalIconButton({ pc(PcCommand.Volume(VolumeAction.MUTE)) }) { Icon(Icons.AutoMirrored.Outlined.VolumeOff, "Sessiz") }
            }
        }

        Section("Sunum") {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                FilledTonalButton({ key(SpecialKey.LEFT) }, Modifier.weight(1f).height(64.dp)) { Icon(Icons.Outlined.ChevronLeft, null); Text("Geri") }
                FilledTonalButton({ key(SpecialKey.RIGHT) }, Modifier.weight(1f).height(64.dp)) { Text("İleri"); Icon(Icons.Outlined.ChevronRight, null) }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedButton({ key(SpecialKey.F5) }, Modifier.weight(1f)) { Text("Başlat (F5)") }
                OutlinedButton({ key(SpecialKey.ESCAPE) }, Modifier.weight(1f)) { Text("Bitir (Esc)") }
            }
        }

        Section("Güç") {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton({ pc(PcCommand.Power(PowerAction.LOCK)) }, Modifier.weight(1f)) { Text("Kilitle") }
                OutlinedButton({ confirm = PowerAction.SLEEP }, Modifier.weight(1f)) { Text("Uyku") }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton({ confirm = PowerAction.RESTART }, Modifier.weight(1f)) { Text("Yeniden başlat") }
                OutlinedButton({ confirm = PowerAction.SHUTDOWN }, Modifier.weight(1f)) { Text("Kapat") }
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
            title = { Text("Bilgisayar $name mı?") },
            text = { Text("Kaydedilmemiş işler kaybolabilir.") },
            confirmButton = { TextButton({ pc(PcCommand.Power(a)); confirm = null }) { Text("Evet") } },
            dismissButton = { TextButton({ confirm = null }) { Text("Vazgeç") } },
        )
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
            content()
        }
    }
}

@Composable
private fun Chip(label: String, onClick: () -> Unit) {
    OutlinedButton(onClick, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) { Text(label) }
}

/** Yazılan metin anında PC'ye gider: eklenen karakterler metin olarak, silinenler Backspace olarak. */
@Composable
private fun LiveKeyboard() {
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
        placeholder = { Text("Buraya yaz, anında bilgisayara gider") },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
        keyboardActions = KeyboardActions(onSend = { key(SpecialKey.ENTER); text = "" }),
        singleLine = true,
    )
}

/** Touchpad: göreli hareket (ivmeli), dokun/çift parmak tık, iki parmak kaydırma, basılı tut-sürükle. */
@Composable
private fun Touchpad(sens: Float) {
    val view = LocalView.current
    Box(
        Modifier
            .fillMaxWidth()
            .height(300.dp)
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(18.dp))
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
        Text("Touchpad", color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f))
    }
}
