package app.kopru.ui

import androidx.annotation.StringRes
import app.kopru.R
import androidx.compose.ui.res.stringResource
import android.content.Context
import android.os.SystemClock
import android.view.HapticFeedbackConstants
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.foundation.layout.aspectRatio
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
import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import app.kopru.CameraService
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.kopru.Core
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material.icons.outlined.Bedtime
import androidx.compose.material.icons.outlined.Keyboard
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.MusicNote
import androidx.compose.material.icons.outlined.PowerSettingsNew
import androidx.compose.material.icons.outlined.RestartAlt
import androidx.compose.material.icons.outlined.Slideshow
import androidx.compose.material.icons.outlined.TouchApp
import androidx.compose.material.icons.outlined.Power
import androidx.compose.material3.Icon
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.defaultMinSize
import uniffi.kopru_core.KeyCode
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

/** Kumanda sayfaları (üstteki segmentli seçici). */
private enum class Page(@StringRes val label: Int, val icon: ImageVector) {
    PAD(R.string.remote_page_pad, Icons.Outlined.TouchApp),
    KEYS(R.string.remote_page_keys, Icons.Outlined.Keyboard),
    MEDIA(R.string.remote_page_media, Icons.Outlined.MusicNote),
    SLIDES(R.string.remote_page_slides, Icons.Outlined.Slideshow),
    POWER(R.string.remote_page_power, Icons.Outlined.PowerSettingsNew),
}

/**
 * Kumanda: üstte segmentli seçici, altta tek odaklı sayfa (büyük hedefler). Son seçilen sayfa hatırlanır; varsayılan Touchpad.
 * Ekran görüntüleme ve kamera artık kendi sekmelerinde (Ekran, Kamera).
 */
@Composable
fun RemoteScreen(ui: UiState) {
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("remote", Context.MODE_PRIVATE) }
    var page by rememberSaveable { mutableStateOf(Page.entries.firstOrNull { it.name == prefs.getString("page", null) } ?: Page.PAD) }
    val on = ui.conn == ConnState.CONNECTED
    val c = tb

    // Klavye sayfasından çıkınca/sekmeden ayrılınca yapışkan değiştiriciler bırakılır.
    DisposableEffect(page) { onDispose { if (page == Page.KEYS) KeyPad.releaseAll() } }
    DisposableEffect(Unit) { onDispose { KeyPad.releaseAll() } }

    Column(Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.remote_title), Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Pill(if (on) stringResource(R.string.remote_pill_live) else stringResource(R.string.remote_pill_offline), if (on) "violet" else "muted")
        }
        Segments(page) { page = it; prefs.edit().putString("page", it.name).apply() }
        if (!on) {
            Text(stringResource(R.string.remote_offline_hint), color = c.faint, style = MaterialTheme.typography.bodySmall)
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (page) {
                Page.PAD -> PadPage(prefs)
                Page.KEYS -> KeysPage()
                Page.MEDIA -> MediaPage()
                Page.SLIDES -> SlidesPage()
                Page.POWER -> PowerPage()
            }
        }
    }
}

/** Segmentli seçici: 56 dp yüksek, ikon + etiket, seçili segment altın. */
@Composable
private fun Segments(selected: Page, onSelect: (Page) -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(16.dp)
    val scroll = rememberScrollState()
    val density = LocalDensity.current
    LaunchedEffect(selected) { scroll.animateScrollTo(with(density) { (selected.ordinal * 76).dp.roundToPx() }) }
    Row(
        Modifier.fillMaxWidth().clip(shape).background(c.s2).border(1.dp, c.line, shape).horizontalScroll(scroll).padding(4.dp).selectableGroup(),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Page.entries.forEach { p ->
            val sel = p == selected
            val pLabel = stringResource(p.label)
            Column(
                Modifier
                    .width(72.dp)
                    .height(56.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(if (sel) c.goldSoft else Color.Transparent)
                    .then(if (sel) Modifier.border(1.dp, c.goldLine, RoundedCornerShape(12.dp)) else Modifier)
                    .selectable(sel, role = Role.Tab, onClick = { onSelect(p) })
                    .semantics { contentDescription = pLabel },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Icon(p.icon, null, tint = if (sel) c.goldHi else c.muted, modifier = Modifier.size(22.dp))
                Spacer(Modifier.height(2.dp))
                Text(pLabel, color = if (sel) c.goldHi else c.text2, style = MaterialTheme.typography.labelSmall, maxLines = 1)
            }
        }
    }
}

/** Touchpad sayfası: sayfayı dolduran büyük pad + tık düğmeleri + hız. */
@Composable
private fun PadPage(prefs: android.content.SharedPreferences) {
    val c = tb
    var sens by remember { mutableFloatStateOf(prefs.getFloat("sens", 1.6f)) }
    var help by remember { mutableStateOf(false) }
    val helpTitle = stringResource(R.string.remote_help)
    @Composable
    fun SpeedControl() {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.remote_speed), color = c.muted, style = MaterialTheme.typography.labelLarge)
            Slider(sens, { sens = it; prefs.edit().putFloat("sens", it).apply() }, valueRange = 0.6f..4f,
                modifier = Modifier.weight(1f),
                colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4))
            TextButton(onClick = { help = true }, modifier = Modifier.size(48.dp).semantics { contentDescription = helpTitle }, contentPadding = PaddingValues(0.dp)) { Text("?") }
        }
    }
    if (help) AlertDialog(onDismissRequest = { help = false }, title = { Text(helpTitle) }, text = { Text(stringResource(R.string.remote_pad_help)) }, confirmButton = { TextButton(onClick = { help = false }) { Text(stringResource(R.string.remote_close)) } })
    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxHeight < 260.dp && maxWidth > maxHeight) {
            Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Touchpad(sens, Modifier.weight(0.6f).fillMaxHeight())
                Column(Modifier.weight(0.4f).fillMaxHeight().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    TbButton(stringResource(R.string.remote_left_click), { pc(PcCommand.Click(MouseButton.LEFT, false)) }, Modifier.fillMaxWidth(), height = 48.dp)
                    TbButton(stringResource(R.string.remote_middle), { pc(PcCommand.Click(MouseButton.MIDDLE, false)) }, Modifier.fillMaxWidth(), height = 48.dp)
                    TbButton(stringResource(R.string.remote_right_click), { pc(PcCommand.Click(MouseButton.RIGHT, false)) }, Modifier.fillMaxWidth(), height = 48.dp)
                    SpeedControl()
                }
            }
        } else {
            Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Touchpad(sens, Modifier.weight(1f))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TbButton(stringResource(R.string.remote_left_click), { pc(PcCommand.Click(MouseButton.LEFT, false)) }, Modifier.weight(1f), height = 48.dp)
                    TbButton(stringResource(R.string.remote_middle), { pc(PcCommand.Click(MouseButton.MIDDLE, false)) }, Modifier.weight(1f), height = 48.dp)
                    TbButton(stringResource(R.string.remote_right_click), { pc(PcCommand.Click(MouseButton.RIGHT, false)) }, Modifier.weight(1f), height = 48.dp)
                }
                SpeedControl()
            }
        }
    }
}

@Composable
private fun KeysPage() {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        LiveKeyboard()
        TbCard(padding = PaddingValues(0.dp)) { SpecialKeysPanel(maxHeightFraction = 1f) }
    }
}

@Composable
private fun MediaPage() {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(24.dp)) {
        TbCard {
            Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
                RoundIcon(Icons.Outlined.SkipPrevious, stringResource(R.string.remote_prev), { pc(PcCommand.Media(MediaAction.PREVIOUS)) }, size = 64.dp)
                RoundIcon(Icons.Outlined.PlayArrow, stringResource(R.string.remote_play_pause), { pc(PcCommand.Media(MediaAction.PLAY_PAUSE)) }, size = 88.dp, primary = true)
                RoundIcon(Icons.Outlined.SkipNext, stringResource(R.string.remote_next), { pc(PcCommand.Media(MediaAction.NEXT)) }, size = 64.dp)
            }
        }
        TbCard {
            SectionTitle(stringResource(R.string.remote_volume))
            Spacer(Modifier.height(12.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TbButton(stringResource(R.string.remote_vol_down), { pc(PcCommand.Volume(VolumeAction.DOWN)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeDown, height = 64.dp)
                TbButton(stringResource(R.string.remote_mute), { pc(PcCommand.Volume(VolumeAction.MUTE)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeOff, height = 64.dp)
                TbButton(stringResource(R.string.remote_vol_up), { pc(PcCommand.Volume(VolumeAction.UP)) }, Modifier.weight(1f), icon = Icons.AutoMirrored.Outlined.VolumeUp, height = 64.dp)
            }
        }
    }
}

@Composable
private fun SlidesPage() {
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TbButton(stringResource(R.string.remote_slide_back), { key(SpecialKey.LEFT) }, Modifier.weight(1f).fillMaxHeight().heightIn(min = 96.dp), icon = Icons.Outlined.ChevronLeft, height = 96.dp)
            TbButton(stringResource(R.string.remote_slide_next), { key(SpecialKey.RIGHT) }, Modifier.weight(1f).fillMaxHeight().heightIn(min = 96.dp), icon = Icons.Outlined.ChevronRight, height = 96.dp)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TbButton(stringResource(R.string.remote_slide_start), { key(SpecialKey.F5) }, Modifier.weight(1f), height = 64.dp)
            TbButton(stringResource(R.string.remote_slide_end), { key(SpecialKey.ESCAPE) }, Modifier.weight(1f), height = 64.dp)
        }
    }
}

@Composable
private fun PowerPage() {
    val c = tb
    var confirm by remember { mutableStateOf<PowerAction?>(null) }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.remote_power_note), color = c.faint, style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            PowerTile(stringResource(R.string.remote_lock), Icons.Outlined.Lock, false, Modifier.weight(1f)) { confirm = PowerAction.LOCK }
            PowerTile(stringResource(R.string.remote_sleep), Icons.Outlined.Bedtime, false, Modifier.weight(1f)) { confirm = PowerAction.SLEEP }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            PowerTile(stringResource(R.string.remote_restart), Icons.Outlined.RestartAlt, true, Modifier.weight(1f)) { confirm = PowerAction.RESTART }
            PowerTile(stringResource(R.string.remote_shutdown), Icons.Outlined.PowerSettingsNew, true, Modifier.weight(1f)) { confirm = PowerAction.SHUTDOWN }
        }
    }
    confirm?.let { a ->
        val title = stringResource(
            when (a) {
                PowerAction.SLEEP -> R.string.remote_confirm_sleep
                PowerAction.RESTART -> R.string.remote_confirm_restart
                PowerAction.SHUTDOWN -> R.string.remote_confirm_shutdown
                PowerAction.LOCK -> R.string.remote_confirm_lock
            },
        )
        AlertDialog(
            onDismissRequest = { confirm = null },
            containerColor = c.s2,
            title = { Text(title) },
            text = { Text(stringResource(if (a == PowerAction.LOCK) R.string.remote_confirm_lock_text else R.string.remote_confirm_loss_text), color = c.text2) },
            confirmButton = { TextButton({ pc(PcCommand.Power(a)); confirm = null }, Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.remote_yes), color = if (a == PowerAction.LOCK) c.goldHi else c.bad) } },
            dismissButton = { TextButton({ confirm = null }, Modifier.heightIn(min = 48.dp)) { Text(stringResource(R.string.remote_cancel), color = c.text2) } },
        )
    }
}

@Composable
private fun PowerTile(label: String, icon: ImageVector, danger: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(16.dp)
    Column(
        modifier
            .heightIn(min = 112.dp)
            .clip(shape)
            .background(if (danger) c.badSoft else c.s2)
            .border(1.dp, if (danger) c.bad.copy(alpha = 0.3f) else c.line2, shape)
            .clickable(onClick = onClick)
            .semantics { contentDescription = label }
            .padding(12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(icon, null, tint = if (danger) c.bad else c.goldHi, modifier = Modifier.size(32.dp))
        Spacer(Modifier.height(8.dp))
        Text(label, color = if (danger) c.bad else c.text, style = MaterialTheme.typography.titleSmall, textAlign = TextAlign.Center)
    }
}

@Composable
internal fun Chip(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(12.dp)
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clip(shape)
            .alpha(if (enabled) 1f else 0.55f)
            .background(c.s3)
            .border(1.dp, c.line2, shape)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = c.text, style = MaterialTheme.typography.labelMedium)
    }
}

/** Touchpad: göreli hareket (ivmeli), dokun/çift parmak tık, iki parmak kaydırma, basılı tut-sürükle. */
@Composable
internal fun Touchpad(sens: Float, modifier: Modifier = Modifier) {
    val view = LocalView.current
    val c = tb
    val shape = RoundedCornerShape(16.dp)
    val scope = rememberCoroutineScope()
    val ripple = remember { Animatable(1f) }
    var rippleAt by remember { mutableStateOf(Offset.Zero) }
    val padDescription = stringResource(R.string.remote_pad_help)
    Box(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(c.cardBrush)
            .border(1.dp, c.goldEdge, shape)
            .semantics { contentDescription = padDescription }
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
                    try {
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
                    } finally {
                        // İptal/kopma/istisna dahil: sol tuş PC'de basılı kalmasın.
                        if (dragging) pc(PcCommand.Button(MouseButton.LEFT, false))
                    }
                    val dur = SystemClock.uptimeMillis() - t0
                    if (!dragging && moved < slop * 2 && dur < 300) {
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
        Text(stringResource(R.string.remote_page_pad), color = c.faint, style = MaterialTheme.typography.labelSmall)
    }
}
