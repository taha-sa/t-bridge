@file:OptIn(ExperimentalFoundationApi::class)

package app.kopru.ui

import app.kopru.R
import androidx.compose.ui.res.stringResource
import android.os.SystemClock
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.Core
import kotlinx.coroutines.withTimeoutOrNull
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.KeyCode
import uniffi.kopru_core.PcCommand
import uniffi.kopru_core.SpecialKey
import kotlin.math.min

/** Yapışkan değiştirici durumu: KAPALI · SIRADAKİ (bir sonraki tuşa/tıka uygulanır) · KİLİTLİ (açılana kadar basılı). */
enum class ModState { OFF, ARMED, LOCKED }

/**
 * Özel tuş mantığı. `key-hold` duyuran PC'de değiştiriciler gerçekten basılı tutulur (KeyDown/KeyUp); bir sonraki tuşa
 * ya da fare tıkına uygulanıp bırakılır (SIRADAKİ) veya kilitli kalır. Eski PC'de (yetenek yok) değiştiriciler devre dışıdır
 * ve tuşlar mevcut Key/kısayol mesajlarına döner; karşılığı olmayanlar (F tuşları, Win+D…) pasif görünür.
 */
object KeyPad {
    val mods = mutableStateMapOf<KeyCode, ModState>()
    private val armedAt = HashMap<KeyCode, Long>()
    private const val DOUBLE_TAP_MS = 450L

    fun holdSupported(): Boolean = Core.node?.peerSupportsKeyHold() == true

    fun state(k: KeyCode): ModState = mods[k] ?: ModState.OFF

    private fun setState(k: KeyCode, s: ModState) {
        val old = state(k)
        if (old == s) return
        if (old == ModState.OFF) Core.node?.sendKey(k, true)
        if (s == ModState.OFF) Core.node?.sendKey(k, false)
        if (s == ModState.OFF) mods.remove(k) else mods[k] = s
    }

    /** Dokun: SIRADAKİ; hızlı ikinci dokunuş: KİLİTLİ; kilitliyken ya da geç ikinci dokunuş: KAPALI. */
    fun tapMod(k: KeyCode) {
        if (!holdSupported()) return
        when (state(k)) {
            ModState.OFF -> { setState(k, ModState.ARMED); armedAt[k] = SystemClock.uptimeMillis() }
            ModState.ARMED -> setState(k, if (SystemClock.uptimeMillis() - (armedAt[k] ?: 0L) < DOUBLE_TAP_MS) ModState.LOCKED else ModState.OFF)
            ModState.LOCKED -> setState(k, ModState.OFF)
        }
    }

    /** Uzun bas: kilitle / kilidi aç. */
    fun longPressMod(k: KeyCode) {
        if (!holdSupported()) return
        setState(k, if (state(k) == ModState.LOCKED) ModState.OFF else ModState.LOCKED)
    }

    /** Bir tuş/tık gönderildi: SIRADAKİ değiştiriciler bırakılır, KİLİTLİ olanlar kalır. */
    fun afterKey() {
        mods.filterValues { it == ModState.ARMED }.keys.forEach { setState(it, ModState.OFF) }
    }

    /** Hepsini bırak (ekrandan/klavye sayfasından çıkış). PC de bağlantı kopunca/ekran durunca kendisi bırakır. */
    fun releaseAll() {
        mods.keys.toList().forEach { setState(it, ModState.OFF) }
    }

    /** Bağlantı koptu: PC zaten bıraktı; yalnızca yerel durumu sıfırla. */
    fun resetLocal() {
        mods.clear()
        armedAt.clear()
    }

    fun available(fallback: SpecialKey?): Boolean = holdSupported() || fallback != null

    /** Tuşa bas-bırak. `key-hold` yoksa `fallback` Key mesajı; o da yoksa false. */
    fun press(k: KeyCode, fallback: SpecialKey? = null): Boolean {
        val n = Core.node ?: return false
        val ok = when {
            holdSupported() -> { n.sendKey(k, true); n.sendKey(k, false); true }
            fallback != null -> { n.sendPc(PcCommand.Key(fallback)); true }
            else -> false
        }
        if (ok) afterKey()
        return ok
    }

    /** Kısayol (örn. Ctrl+C): zaten basılı değiştiriciler korunur, eklenenler bırakılır. */
    fun combo(modKeys: List<KeyCode>, k: KeyCode, fallback: SpecialKey? = null): Boolean {
        val n = Core.node ?: return false
        if (!holdSupported()) {
            if (fallback == null) return false
            n.sendPc(PcCommand.Key(fallback))
            return true
        }
        val added = modKeys.filter { state(it) == ModState.OFF }
        added.forEach { n.sendKey(it, true) }
        n.sendKey(k, true)
        n.sendKey(k, false)
        added.asReversed().forEach { n.sendKey(it, false) }
        afterKey()
        return true
    }

    private fun letterKey(ch: Char): KeyCode? = when (ch.uppercaseChar()) {
        'A' -> KeyCode.A; 'C' -> KeyCode.C; 'D' -> KeyCode.D; 'E' -> KeyCode.E; 'L' -> KeyCode.L; 'R' -> KeyCode.R
        'S' -> KeyCode.S; 'V' -> KeyCode.V; 'X' -> KeyCode.X; 'Y' -> KeyCode.Y; 'Z' -> KeyCode.Z
        else -> null
    }

    /** Yazılan metin. Bir değiştirici (Ctrl…) etkinken harf kısayolu olur (Ctrl+C); aksi halde düz Unicode metin. */
    fun typeText(s: String) {
        val n = Core.node ?: return
        val buf = StringBuilder()
        fun flush() {
            if (buf.isNotEmpty()) { n.sendPc(PcCommand.Text(buf.toString())); buf.clear() }
        }
        for (ch in s) {
            val k = if (mods.isNotEmpty() && holdSupported()) letterKey(ch) else null
            if (k != null) { flush(); press(k) } else buf.append(ch)
        }
        flush()
    }
}

/** Tek tuş düğmesi: 48 dp yüksek, yapışkan durum görseli, isteğe bağlı otomatik tekrar (oklar, silme). */
@Composable
internal fun KeyBtn(
    label: String,
    modifier: Modifier = Modifier,
    desc: String = label,
    enabled: Boolean = true,
    mod: ModState? = null,
    repeat: Boolean = false,
    onLong: (() -> Unit)? = null,
    onClick: () -> Unit,
) {
    val c = tb
    val shape = RoundedCornerShape(12.dp)
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    var rpPressed by remember { mutableStateOf(false) }
    val down = pressed || rpPressed
    val (bg, border, fg) = when {
        !enabled -> Triple(c.s2, c.line, c.faint)
        mod == ModState.LOCKED -> Triple(c.gold, c.goldHi, c.onGold)
        mod == ModState.ARMED -> Triple(c.goldSoft, c.goldHi, c.goldHi)
        down -> Triple(c.s4, c.goldLine, c.text)
        else -> Triple(c.s3, c.line2, c.text)
    }
    val stateText = when (mod) {
        ModState.LOCKED -> stringResource(R.string.keys_state_locked)
        ModState.ARMED -> stringResource(R.string.keys_state_armed)
        else -> null
    }
    val base = modifier
        .height(48.dp)
        .defaultMinSize(minWidth = 48.dp)
        .clip(shape)
        .background(bg)
        .border(1.dp, border, shape)
        .semantics {
            contentDescription = desc
            role = Role.Button
            if (stateText != null) stateDescription = stateText
        }
    val gesture = when {
        !enabled -> Modifier
        repeat -> Modifier.pointerInput(onClick) {
            awaitEachGesture {
                awaitFirstDown(requireUnconsumed = false).consume()
                rpPressed = true
                onClick()
                var wait = 420L
                var held = true
                while (held) {
                    val ev = withTimeoutOrNull(wait) { awaitPointerEvent() }
                    if (ev == null) {
                        onClick()
                        wait = 55L
                    } else {
                        ev.changes.forEach { it.consume() }
                        if (ev.changes.none { it.pressed }) held = false
                    }
                }
                rpPressed = false
            }
        }
        else -> Modifier.combinedClickable(interactionSource = src, indication = null, onLongClick = onLong, onClick = onClick)
    }
    Box(base.then(gesture), contentAlignment = Alignment.Center) {
        Text(
            label,
            color = fg,
            style = MaterialTheme.typography.labelLarge,
            fontWeight = if (mod != null && mod != ModState.OFF) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 6.dp),
        )
        if (mod == ModState.LOCKED) {
            // Kilit göstergesi: küçük nokta (renge ek olarak biçimle de ayırt edilir).
            Box(Modifier.align(Alignment.TopEnd).padding(5.dp).size(6.dp).clip(RoundedCornerShape(3.dp)).background(c.onGold))
        }
    }
}

@Composable
private fun KeyRow(content: @Composable RowScope.() -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), content = content)
}

/**
 * Özel tuş takımı (Ekran'da ve Kumanda → Klavye'de): yapışkan Ctrl/Alt/Shift/Win, Esc/Tab/Enter/Sil, oklar, Home/End,
 * PgUp/PgDn, kaydırmalı F1–F12 ve kısayollar. Yüksekliği sınırlıdır (ekranın yarısı), taşarsa kaydırılır.
 */
@Composable
internal fun SpecialKeysPanel(modifier: Modifier = Modifier, maxHeightFraction: Float = 0.5f) {
    val ui by Core.ui.collectAsStateWithLifecycle()
    val connected = ui.conn == ConnState.CONNECTED
    val hold = connected && KeyPad.holdSupported()
    val c = tb
    val maxH = (LocalConfiguration.current.screenHeightDp * maxHeightFraction).dp

    @Composable
    fun RowScope.K(
        label: String,
        k: KeyCode,
        fb: SpecialKey?,
        desc: String = label,
        repeat: Boolean = false,
        weight: Float = 1f,
    ) {
        KeyBtn(label, Modifier.weight(weight), desc, enabled = connected && (hold || fb != null), repeat = repeat) { KeyPad.press(k, fb) }
    }

    @Composable
    fun RowScope.M(label: String, k: KeyCode, desc: String) {
        KeyBtn(
            label, Modifier.weight(1f), desc,
            enabled = hold, mod = KeyPad.state(k),
            onLong = { KeyPad.longPressMod(k) },
        ) { KeyPad.tapMod(k) }
    }

    Column(modifier.fillMaxWidth()) {
        Column(
            Modifier.fillMaxWidth().heightIn(max = maxH).verticalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (connected && !hold) {
                Text(
                    stringResource(R.string.keys_old_pc),
                    color = c.muted,
                    style = MaterialTheme.typography.bodySmall,
                )
            } else {
                Text(
                    stringResource(R.string.keys_modifier_help),
                    color = c.muted,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            KeyRow {
                M("Ctrl", KeyCode.CTRL, "Control")
                M("Alt", KeyCode.ALT, "Alt")
                M("Shift", KeyCode.SHIFT, "Shift")
                M("Win", KeyCode.WIN, stringResource(R.string.keys_desc_win))
            }
            KeyRow {
                K("Esc", KeyCode.ESC, SpecialKey.ESCAPE, "Escape")
                K("Tab", KeyCode.TAB, SpecialKey.TAB, "Tab")
                K("Enter", KeyCode.ENTER, SpecialKey.ENTER, "Enter")
                K("⌫", KeyCode.BACKSPACE, SpecialKey.BACKSPACE, stringResource(R.string.keys_desc_backspace), repeat = true)
                K("Del", KeyCode.DELETE, SpecialKey.DELETE, "Delete")
                K("Ins", KeyCode.INSERT, null, "Insert")
            }
            KeyRow {
                K("←", KeyCode.LEFT, SpecialKey.LEFT, stringResource(R.string.keys_desc_left), repeat = true)
                K("↑", KeyCode.UP, SpecialKey.UP, stringResource(R.string.keys_desc_up), repeat = true)
                K("↓", KeyCode.DOWN, SpecialKey.DOWN, stringResource(R.string.keys_desc_down), repeat = true)
                K("→", KeyCode.RIGHT, SpecialKey.RIGHT, stringResource(R.string.keys_desc_right), repeat = true)
                K("Home", KeyCode.HOME, SpecialKey.HOME, "Home")
                K("End", KeyCode.END, SpecialKey.END, "End")
            }
            KeyRow {
                K("PgUp", KeyCode.PAGE_UP, SpecialKey.PAGE_UP, stringResource(R.string.keys_desc_pgup), repeat = true)
                K("PgDn", KeyCode.PAGE_DOWN, SpecialKey.PAGE_DOWN, stringResource(R.string.keys_desc_pgdn), repeat = true)
                KeyBtn("Alt+Tab", Modifier.weight(1.3f), stringResource(R.string.keys_desc_alttab), enabled = connected && KeyPad.available(SpecialKey.ALT_TAB)) {
                    KeyPad.combo(listOf(KeyCode.ALT), KeyCode.TAB, SpecialKey.ALT_TAB)
                }
                KeyBtn("Win+D", Modifier.weight(1.3f), stringResource(R.string.keys_desc_windesk), enabled = hold) {
                    KeyPad.combo(listOf(KeyCode.WIN), KeyCode.D)
                }
            }
            // F1–F12: kaydırmalı tek sıra
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val fk = listOf(
                    KeyCode.F1, KeyCode.F2, KeyCode.F3, KeyCode.F4, KeyCode.F5, KeyCode.F6,
                    KeyCode.F7, KeyCode.F8, KeyCode.F9, KeyCode.F10, KeyCode.F11, KeyCode.F12,
                )
                fk.forEachIndexed { i, k ->
                    val fb = if (k == KeyCode.F5) SpecialKey.F5 else null
                    KeyBtn("F${i + 1}", Modifier.width(56.dp), "F${i + 1}", enabled = connected && (hold || fb != null)) { KeyPad.press(k, fb) }
                }
            }
            KeyRow {
                KeyBtn(stringResource(R.string.keys_copy), Modifier.weight(1f), stringResource(R.string.keys_copy_desc), enabled = connected && KeyPad.available(SpecialKey.COPY_SHORTCUT)) {
                    KeyPad.combo(listOf(KeyCode.CTRL), KeyCode.C, SpecialKey.COPY_SHORTCUT)
                }
                KeyBtn(stringResource(R.string.keys_paste), Modifier.weight(1f), stringResource(R.string.keys_paste_desc), enabled = connected && KeyPad.available(SpecialKey.PASTE_SHORTCUT)) {
                    KeyPad.combo(listOf(KeyCode.CTRL), KeyCode.V, SpecialKey.PASTE_SHORTCUT)
                }
                KeyBtn(stringResource(R.string.keys_undo), Modifier.weight(1f), stringResource(R.string.keys_undo_desc), enabled = hold) {
                    KeyPad.combo(listOf(KeyCode.CTRL), KeyCode.Z)
                }
            }
        }
    }
}

/**
 * Yazılan metin anında PC'ye gider: eklenen karakterler metin olarak, silinenler Backspace olarak.
 * Bir değiştirici etkinken (Ctrl vb.) yazılan harf kısayol olur.
 */
@Composable
internal fun LiveKeyboard(modifier: Modifier = Modifier, autoFocus: Boolean = false) {
    var text by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    val kb = LocalSoftwareKeyboardController.current
    if (autoFocus) {
        LaunchedEffect(Unit) {
            focus.requestFocus()
            kb?.show()
        }
        DisposableEffect(Unit) { onDispose { kb?.hide() } }
    }
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
            repeat(deleted) { Core.node?.sendPc(PcCommand.Key(SpecialKey.BACKSPACE)) }
            val added = new.substring(p)
            if (added.isNotEmpty()) KeyPad.typeText(added)
            // Uzun tamponu kısa tut (diff ucuz kalsın).
            text = if (new.length > 200) "" else new
        },
        modifier = modifier.fillMaxWidth().focusRequester(focus),
        placeholder = { Text(stringResource(R.string.keys_placeholder), color = tb.faint) },
        shape = RoundedCornerShape(14.dp),
        colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = tb.gold, unfocusedBorderColor = tb.line2, cursorColor = tb.goldHi),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
        keyboardActions = KeyboardActions(onSend = { KeyPad.press(KeyCode.ENTER, SpecialKey.ENTER); text = "" }),
        singleLine = true,
    )
}
