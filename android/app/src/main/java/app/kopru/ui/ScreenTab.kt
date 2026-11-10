@file:OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)

package app.kopru.ui

import app.kopru.R
import androidx.compose.ui.res.stringResource
import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Cast
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Fullscreen
import androidx.compose.material.icons.outlined.FullscreenExit
import androidx.compose.material.icons.outlined.Keyboard
import androidx.compose.material.icons.outlined.KeyboardAlt
import androidx.compose.material.icons.outlined.Mouse
import androidx.compose.material.icons.outlined.Speed
import androidx.compose.material.icons.outlined.TouchApp
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.Screen
import app.kopru.ScreenPreset
import app.kopru.UiState
import uniffi.kopru_core.ConnState

/** Uygulama düzeyi görünüm durumu: Ekran sekmesinde tam ekran (sistem çubukları + alt gezinme gizli). */
object Chrome {
    var immersive by mutableStateOf(false)
}

private val ToolbarHeight = 56.dp

/**
 * Ekran sekmesi: tam taşan PC görüntüsü + kalıcı ince alt araç çubuğu (mod · klavye · tuşlar · kalite · tam ekran).
 * Klavye ya da özel tuş satırı açılınca görüntü alanı küçülür (letterbox), yayın ve oturum sürer: PC'ye bildirilen görüntü
 * alanı klavyesiz tam boydur, bu yüzden yeniden kodlama olmaz. Dikey ve yatay desteklenir; döndürme oturumu bozmaz.
 */
@Composable
fun ScreenTab(ui: UiState) {
    val c = tb
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("remote", Context.MODE_PRIVATE) }
    val scr by Screen.ui.collectAsStateWithLifecycle()
    val on = ui.conn == ConnState.CONNECTED
    var mode by rememberSaveable { mutableStateOf(ControlMode.entries.firstOrNull { it.name == prefs.getString("screen_mode", null) } ?: ControlMode.TAP) }
    var kbOpen by rememberSaveable { mutableStateOf(false) }
    var keysOpen by rememberSaveable { mutableStateOf(false) }
    var qualityOpen by rememberSaveable { mutableStateOf(false) }
    val imeUp = WindowInsets.isImeVisible
    val density = LocalDensity.current

    ScreenLifecycle()

    // Sistem geri hareketiyle klavye kapandıysa yazı satırını da kapat.
    var sawIme by remember { mutableStateOf(false) }
    LaunchedEffect(imeUp, kbOpen) {
        if (kbOpen) {
            if (imeUp) sawIme = true else if (sawIme) { kbOpen = false; sawIme = false }
        } else sawIme = false
    }

    if (!scr.viewing) {
        StartPanel(scr, on)
        return
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(Color.Black)
            .onSizeChanged { sz ->
                // PC'ye bildirilen alan: klavyesiz tam yükseklik − araç çubuğu. Klavye açıkken güncellenmez (yeniden kodlama yok).
                if (!imeUp) Screen.setViewport(sz.width, sz.height - with(density) { ToolbarHeight.roundToPx() })
            },
    ) {
        Box(Modifier.weight(1f).fillMaxWidth()) {
            ScreenViewer(scr, mode, onClose = { Screen.stopViewing() }, modifier = Modifier.fillMaxSize())
        }
        AnimatedVisibility(keysOpen, enter = expandVertically(), exit = shrinkVertically()) {
            Column(Modifier.fillMaxWidth().background(Color(0xFF0E0E10))) {
                SpecialKeysPanel(maxHeightFraction = 0.42f)
            }
        }
        AnimatedVisibility(kbOpen, enter = expandVertically(), exit = shrinkVertically()) {
            Box(Modifier.fillMaxWidth().background(Color(0xFF0E0E10)).padding(horizontal = 12.dp, vertical = 8.dp)) {
                LiveKeyboard(autoFocus = true)
            }
        }
        Toolbar(
            mode = mode,
            kbOpen = kbOpen,
            keysOpen = keysOpen,
            full = Chrome.immersive,
            onMode = {
                mode = if (mode == ControlMode.TAP) ControlMode.PAD else ControlMode.TAP
                prefs.edit().putString("screen_mode", mode.name).apply()
            },
            onKeyboard = { kbOpen = !kbOpen },
            onKeys = { keysOpen = !keysOpen },
            onQuality = { qualityOpen = true },
            onFull = { Chrome.immersive = !Chrome.immersive },
        )
    }

    if (qualityOpen) QualitySheet(scr.preset, onPick = { Screen.setPreset(it); qualityOpen = false }, onDismiss = { qualityOpen = false })
}

@Composable
private fun Toolbar(
    mode: ControlMode,
    kbOpen: Boolean,
    keysOpen: Boolean,
    full: Boolean,
    onMode: () -> Unit,
    onKeyboard: () -> Unit,
    onKeys: () -> Unit,
    onQuality: () -> Unit,
    onFull: () -> Unit,
) {
    val c = tb
    Column(Modifier.fillMaxWidth().background(Color(0xFF0E0E10))) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.goldLine))
        Row(Modifier.fillMaxWidth().height(ToolbarHeight), verticalAlignment = Alignment.CenterVertically) {
            val pad = mode == ControlMode.PAD
            ToolItem(
                if (pad) Icons.Outlined.Mouse else Icons.Outlined.TouchApp,
                stringResource(mode.label),
                stringResource(if (pad) R.string.scr_mode_to_tap else R.string.scr_mode_to_pad, stringResource(mode.label)),
                active = false,
                onClick = onMode,
                Modifier.weight(1f),
            )
            ToolItem(Icons.Outlined.Keyboard, stringResource(R.string.scr_tool_keyboard), stringResource(R.string.scr_tool_keyboard), kbOpen, onKeyboard, Modifier.weight(1f))
            ToolItem(Icons.Outlined.KeyboardAlt, stringResource(R.string.scr_tool_keys), stringResource(R.string.scr_tool_keys_desc), keysOpen, onKeys, Modifier.weight(1f))
            ToolItem(Icons.Outlined.Speed, stringResource(R.string.scr_tool_quality), stringResource(R.string.scr_tool_quality_desc), false, onQuality, Modifier.weight(1f))
            ToolItem(
                if (full) Icons.Outlined.FullscreenExit else Icons.Outlined.Fullscreen,
                stringResource(if (full) R.string.scr_tool_shrink else R.string.scr_full),
                stringResource(if (full) R.string.scr_full_exit else R.string.scr_full),
                full, onFull, Modifier.weight(1f),
            )
        }
    }
}

@Composable
private fun ToolItem(icon: ImageVector, label: String, desc: String, active: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val c = tb
    val fg = if (active) c.goldHi else Color(0xFFF2F0EA)
    val onDesc = stringResource(R.string.scr_state_on)
    Column(
        modifier
            .height(ToolbarHeight)
            .clickable(onClick = onClick)
            .semantics(mergeDescendants = true) {
                role = Role.Button
                contentDescription = desc
                if (active) stateDescription = onDesc
            },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(Modifier.clip(RoundedCornerShape(10.dp)).background(if (active) c.goldSoft else Color.Transparent).padding(horizontal = 14.dp, vertical = 3.dp)) {
            Icon(icon, null, tint = fg, modifier = Modifier.size(22.dp))
        }
        Text(label, color = fg, style = MaterialTheme.typography.labelSmall, maxLines = 1, textAlign = TextAlign.Center)
    }
}

private val presetNotes = mapOf(
    ScreenPreset.ECO to R.string.scr_preset_eco_note,
    ScreenPreset.BALANCED to R.string.scr_preset_balanced_note,
    ScreenPreset.SMOOTH to R.string.scr_preset_smooth_note,
)

@Composable
private fun QualitySheet(current: ScreenPreset, onPick: (ScreenPreset) -> Unit, onDismiss: () -> Unit) {
    val c = tb
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(), containerColor = c.s1) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.scr_quality_title), color = c.text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(bottom = 4.dp))
            ScreenPreset.entries.forEach { p ->
                val sel = p == current
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(16.dp))
                        .background(if (sel) c.goldSoft else c.s2)
                        .border(1.dp, if (sel) c.goldLine else c.line, RoundedCornerShape(16.dp))
                        .clickable { onPick(p) }
                        .heightIn(min = 64.dp)
                        .padding(horizontal = 16.dp, vertical = 12.dp)
                        .semantics(mergeDescendants = true) { role = Role.RadioButton; selected = sel },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(p.label), color = if (sel) c.goldHi else c.text, style = MaterialTheme.typography.titleSmall)
                        Text(presetNotes[p]?.let { stringResource(it) } ?: "", color = c.muted, style = MaterialTheme.typography.bodySmall)
                    }
                    if (sel) Icon(Icons.Outlined.Check, stringResource(R.string.scr_selected), tint = c.goldHi, modifier = Modifier.size(22.dp))
                }
            }
        }
    }
}

/** Yayın kapalıyken: durum + büyük "Ekranı başlat" düğmesi. */
@Composable
private fun StartPanel(scr: app.kopru.ScreenUi, on: Boolean) {
    val c = tb
    val supported = on && Screen.supported()
    Column(
        Modifier.fillMaxSize().statusBarsPadding().padding(horizontal = 16.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.scr_title), Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Pill(if (on) stringResource(R.string.remote_pill_live) else stringResource(R.string.remote_pill_offline), if (on) "violet" else "muted")
        }
        Column(Modifier.weight(1f).fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
            IconTile(Icons.Outlined.Cast, size = 72.dp)
            Spacer(Modifier.height(16.dp))
            Text(stringResource(R.string.scr_pc_screen), color = c.text, style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(8.dp))
            Text(
                when {
                    !on -> stringResource(R.string.scr_hint_offline)
                    !supported -> stringResource(R.string.scr_hint_unsupported)
                    scr.errorRes != 0 -> stringResource(scr.errorRes)
                    scr.error != null -> scr.error
                    else -> stringResource(R.string.scr_hint_ready)
                } ?: "",
                color = if (scr.error != null || scr.errorRes != 0) c.bad else c.muted,
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.widthIn(max = 420.dp),
            )
        }
        GoldButton(stringResource(R.string.scr_start), { Screen.startViewing() }, Modifier.fillMaxWidth(), icon = Icons.Outlined.Cast, enabled = supported, height = 56.dp)
    }
}

/** Uygulama arka plana gidince çözmeyi durdur (pil), dönünce sürdür; sekmeden çıkınca paylaşımı kapat ve basılı tuşları bırak. */
@Composable
private fun ScreenLifecycle() {
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val obs = LifecycleEventObserver { _, e ->
            if (e == Lifecycle.Event.ON_STOP) { Screen.pause(); KeyPad.releaseAll() }
            if (e == Lifecycle.Event.ON_START) Screen.resume()
        }
        owner.lifecycle.addObserver(obs)
        onDispose {
            owner.lifecycle.removeObserver(obs)
            KeyPad.releaseAll()
            Screen.stopViewing()
        }
    }
}
