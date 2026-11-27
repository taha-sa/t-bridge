package app.kopru.ui

import androidx.compose.ui.res.stringResource
import app.kopru.OemAutostart
import app.kopru.R
import android.Manifest
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Computer
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.Upload
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.kopru.Core
import app.kopru.NotifListener
import app.kopru.UiState
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.PcCommand
import uniffi.kopru_core.PowerAction
import uniffi.kopru_core.ServerInfo

@Composable
fun HomeScreen(ui: UiState, go: (Int) -> Unit) {
    val ctx = LocalContext.current
    var pairing by remember { mutableStateOf(false) }
    var codeFor by remember { mutableStateOf<ServerInfo?>(null) }
    var scanning by remember { mutableStateOf(false) }
    val on = ui.conn == ConnState.CONNECTED
    val showPair = (pairing || ui.peers.isEmpty()) && !on
    if (pairing && on) pairing = false

    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 18.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Header()
        Hero(ui)
        ui.startError?.let { ErrorCard(stringResource(R.string.home_core_start_failed, it)) }
        if (on) QuickActions(go)
        if (showPair) {
            PairCard(ui, onScan = { scanning = true }, onPick = { codeFor = it })
        } else {
            PeersCard(ui, onNew = { pairing = true })
        }
        if (!on) HelpCard(defaultOpen = false)
        PermissionsCard()
        Tip()
    }

    if (scanning) {
        QrScanner(onClose = { scanning = false }) { raw ->
            scanning = false
            runCatching { Core.node?.pairUri(raw) }.onFailure {
                Toast.makeText(ctx, ctx.getString(R.string.home_qr_not_tbridge), Toast.LENGTH_LONG).show()
            }
        }
    }

    codeFor?.let { srv ->
        CodeDialog(srv, onDismiss = { codeFor = null }) { code ->
            codeFor = null
            runCatching { Core.node?.pairCode(srv.id, code) }.onFailure {
                Toast.makeText(ctx, it.message, Toast.LENGTH_LONG).show()
            }
        }
    }
    ui.pairError?.let { err ->
        AlertDialog(
            onDismissRequest = { Core.clearPairError() },
            containerColor = tb.s2,
            confirmButton = { TextButton({ Core.clearPairError() }) { Text(stringResource(R.string.home_ok)) } },
            title = { Text(explainPairError(err).first) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    explainPairError(err).second.forEach { Text("• $it", color = tb.text2, style = MaterialTheme.typography.bodyMedium) }
                }
            },
        )
    }
}

@Composable
private fun Header() {
    val c = tb
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp, bottom = 2.dp)) {
        Logo(30.dp)
        Spacer(Modifier.width(10.dp))
        Text("T-Bridge", color = c.text, style = MaterialTheme.typography.titleLarge)
    }
}

@Composable
private fun Hero(ui: UiState) {
    val c = tb
    val on = ui.conn == ConnState.CONNECTED
    val (eyebrow, title, sub) = when (ui.conn) {
        ConnState.CONNECTED -> Triple(stringResource(R.string.home_hero_connected_eyebrow), ui.peer?.name ?: stringResource(R.string.home_hero_computer_default), stringResource(R.string.home_hero_connected_sub))
        ConnState.CONNECTING -> Triple(stringResource(R.string.home_hero_status), stringResource(R.string.home_hero_connecting), stringResource(R.string.home_hero_connecting_sub))
        ConnState.DISCONNECTED ->
            if (ui.peers.isEmpty()) Triple(stringResource(R.string.home_hero_welcome), stringResource(R.string.home_hero_pair_title), stringResource(R.string.home_hero_pair_sub))
            else Triple(stringResource(R.string.home_hero_status), stringResource(R.string.home_hero_disconnected), stringResource(R.string.home_hero_disconnected_sub))
    }
    TbCard(gold = on, padding = androidx.compose.foundation.layout.PaddingValues(20.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            StatusOrb(ui.conn, 72.dp)
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                SectionTitle(eyebrow, color = c.gold)
                Spacer(Modifier.height(3.dp))
                Text(title, color = c.text, style = MaterialTheme.typography.headlineMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.height(3.dp))
                Text(sub, color = c.muted, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}

@Composable
private fun QuickActions(go: (Int) -> Unit) {
    val ctx = LocalContext.current
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        ActionTile(stringResource(R.string.home_action_send_clipboard), Icons.Outlined.ContentPaste, Modifier.weight(1f)) {
            val cm = ctx.getSystemService(ClipboardManager::class.java)
            val text = cm?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(ctx)?.toString()
            val msg = when {
                text.isNullOrEmpty() -> ctx.getString(R.string.home_toast_clip_empty)
                Core.sendClipboardText(text) -> ctx.getString(R.string.home_toast_clip_sent)
                else -> ctx.getString(R.string.home_toast_clip_dup)
            }
            Toast.makeText(ctx, msg, Toast.LENGTH_SHORT).show()
        }
        ActionTile(stringResource(R.string.home_action_send_file), Icons.Outlined.Upload, Modifier.weight(1f)) { go(1) }
        ActionTile(stringResource(R.string.home_action_lock_pc), Icons.Outlined.Lock, Modifier.weight(1f)) {
            Core.node?.sendPc(PcCommand.Power(PowerAction.LOCK))
            Toast.makeText(ctx, ctx.getString(R.string.home_toast_locked), Toast.LENGTH_SHORT).show()
        }
    }
}

@Composable
private fun ActionTile(label: String, icon: ImageVector, modifier: Modifier, onClick: () -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(16.dp)
    Column(
        modifier
            .clip(shape)
            .background(c.cardBrush)
            .border(1.dp, c.line, shape)
            .clickable(onClick = onClick)
            .padding(14.dp),
    ) {
        IconTile(icon, size = 38.dp)
        Spacer(Modifier.height(10.dp))
        Text(label, color = c.text, style = MaterialTheme.typography.titleSmall)
    }
}

/** Çekirdekten gelen teknik hatayı başlık + çözüm adımlarına çevirir. IP/port gösterilmez. */
@Composable
private fun explainPairError(raw: String): Pair<String, List<String>> {
    val r = raw.lowercase()
    return when {
        "kod" in r || "reddedildi" in r -> stringResource(R.string.home_err_code_title) to listOf(
            stringResource(R.string.home_err_code_1),
            stringResource(R.string.home_err_code_2),
        )
        "sürüm" in r -> stringResource(R.string.home_err_version_title) to listOf(stringResource(R.string.home_err_version_1))
        "certificate" in r || "sertifika" in r || "tls" in r || "fingerprint" in r -> stringResource(R.string.home_err_cert_title) to listOf(
            stringResource(R.string.home_err_cert_1),
            stringResource(R.string.home_err_cert_2),
        )
        else -> stringResource(R.string.home_err_unreach_title) to listOf(
            stringResource(R.string.home_err_unreach_1),
            stringResource(R.string.home_err_unreach_2),
            stringResource(R.string.home_err_unreach_3),
            stringResource(R.string.home_err_unreach_4),
        )
    }
}

/** Bağlantı yokken gösterilen çözüm önerileri + hızlı eylemler. */
@Composable
private fun HelpCard(defaultOpen: Boolean) {
    val c = tb
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(defaultOpen) }
    TbCard {
        Row(Modifier.fillMaxWidth().clickable { open = !open }, verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.home_help_title), Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.titleSmall)
            Text(if (open) stringResource(R.string.home_help_close) else stringResource(R.string.home_help_show), color = c.goldHi, style = MaterialTheme.typography.labelLarge)
        }
        if (open) {
            Spacer(Modifier.height(10.dp))
            listOf(
                stringResource(R.string.home_help_1),
                stringResource(R.string.home_help_2),
                stringResource(R.string.home_help_3),
                stringResource(R.string.home_help_4),
                stringResource(R.string.home_help_5),
            ).forEachIndexed { i, s ->
                Row(Modifier.padding(vertical = 4.dp)) {
                    Text("${i + 1}.", Modifier.width(22.dp), color = c.gold, style = MaterialTheme.typography.bodyMedium)
                    Text(s, color = c.text2, style = MaterialTheme.typography.bodyMedium)
                }
            }
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TbButton(stringResource(R.string.home_rescan), {
                    Core.rescan()
                    Toast.makeText(ctx, ctx.getString(R.string.home_toast_rescanning), Toast.LENGTH_SHORT).show()
                }, Modifier.weight(1f), height = 44.dp)
                TbButton(stringResource(R.string.home_wifi_settings), {
                    runCatching { ctx.startActivity(Intent(Settings.ACTION_WIFI_SETTINGS)) }
                }, Modifier.weight(1f), height = 44.dp)
            }
        }
    }
}

@Composable
private fun ErrorCard(msg: String) {
    val c = tb
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.WarningAmber, null, tint = c.bad)
            Spacer(Modifier.width(10.dp))
            Text(msg, color = c.bad, style = MaterialTheme.typography.bodyMedium)
        }
    }
}

@Composable
private fun PairCard(ui: UiState, onScan: () -> Unit, onPick: (ServerInfo) -> Unit) {
    val c = tb
    // Kart açılınca sınırlı bir tarama penceresi aç (zaten tarıyorsa dokunma).
    LaunchedEffect(Unit) { if (!ui.scanning) Core.rescan() }
    TbCard(gold = true) {
        SectionTitle(stringResource(R.string.home_pair_eyebrow), color = c.gold)
        Spacer(Modifier.height(4.dp))
        Text(stringResource(R.string.home_pair_title), color = c.text, style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(4.dp))
        Text(stringResource(R.string.home_pair_desc), color = c.muted, style = MaterialTheme.typography.bodyMedium)
        Spacer(Modifier.height(14.dp))
        GoldButton(stringResource(R.string.home_pair_scan), onScan, Modifier.fillMaxWidth(), icon = Icons.Outlined.QrCodeScanner)
        Spacer(Modifier.height(16.dp))
        SectionTitle(stringResource(R.string.home_pair_or_pick))
        Spacer(Modifier.height(6.dp))
        val list = ui.discovered.filter { !it.paired }
        if (list.isEmpty()) {
            // Sabit metin (dönen animasyon yok): tarama sınırlı bir pencere, bitince "Yeniden tara".
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    if (ui.scanning) stringResource(R.string.home_pair_scanning) else stringResource(R.string.home_pair_none),
                    Modifier.weight(1f), color = c.muted, style = MaterialTheme.typography.bodyMedium,
                )
                if (!ui.scanning) TextButton({ Core.rescan() }) { Text(stringResource(R.string.home_rescan), color = c.goldHi) }
            }
        }
        list.forEach { s ->
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).clickable { onPick(s) }.padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconTile(Icons.Outlined.Computer, size = 38.dp)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(s.name, color = c.text, style = MaterialTheme.typography.titleSmall)
                    Text(stringResource(R.string.home_pair_same_network), color = c.muted, style = MaterialTheme.typography.bodySmall)
                }
                Text(stringResource(R.string.home_pair_enter_code), color = c.goldHi, style = MaterialTheme.typography.labelLarge)
            }
        }
    }
}

@Composable
private fun PeersCard(ui: UiState, onNew: () -> Unit) {
    val c = tb
    var confirm by remember { mutableStateOf<String?>(null) }
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SectionTitle(stringResource(R.string.home_peers_title), Modifier.weight(1f))
            TextButton(onNew) { Text(stringResource(R.string.home_peers_add), color = c.goldHi) }
        }
        ui.peers.forEach { p ->
            val live = ui.peer?.id == p.id
            Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (live) IconTile(Icons.Outlined.Computer, size = 40.dp)
                else IconTile(Icons.Outlined.Computer, tint = c.muted, bg = c.s3, border = c.line, size = 40.dp)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(p.name, Modifier.weight(1f, fill = false), color = c.text, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (live) {
                            Spacer(Modifier.width(8.dp))
                            Pill(stringResource(R.string.home_peer_connected_pill))
                        }
                    }
                    Text(if (live) stringResource(R.string.home_peer_encrypted) else stringResource(R.string.home_peer_paired), color = c.muted, style = MaterialTheme.typography.bodySmall)
                }
                TextButton({ confirm = p.id }) { Text(stringResource(R.string.home_peer_remove), color = c.muted) }
            }
        }
    }
    confirm?.let { id ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            containerColor = c.s2,
            confirmButton = {
                TextButton({
                    Core.node?.unpair(id)
                    Core.refreshPeers()
                    confirm = null
                }) { Text(stringResource(R.string.home_peer_remove), color = c.bad) }
            },
            dismissButton = { TextButton({ confirm = null }) { Text(stringResource(R.string.home_cancel), color = c.text2) } },
            title = { Text(stringResource(R.string.home_unpair_title)) },
            text = { Text(stringResource(R.string.home_unpair_text), color = c.text2) },
        )
    }
}

@Composable
private fun CodeDialog(srv: ServerInfo, onDismiss: () -> Unit, onOk: (String) -> Unit) {
    val c = tb
    var code by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = c.s2,
        title = { Text(stringResource(R.string.home_code_title, srv.name)) },
        text = {
            Column {
                Text(stringResource(R.string.home_code_prompt), color = c.text2)
                Spacer(Modifier.size(10.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { v -> code = v.filter(Char::isDigit).take(6) },
                    singleLine = true,
                    textStyle = MaterialTheme.typography.headlineMedium,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    placeholder = { Text("000000", style = MaterialTheme.typography.headlineMedium, color = c.faint) },
                )
            }
        },
        confirmButton = { TextButton({ onOk(code) }, enabled = code.length == 6) { Text(stringResource(R.string.home_code_confirm), color = if (code.length == 6) c.goldHi else c.faint) } },
        dismissButton = { TextButton(onDismiss) { Text(stringResource(R.string.home_cancel), color = c.text2) } },
    )
}

private data class Perm(val title: String, val desc: String, val ok: Boolean?, val action: (Context) -> Unit)

@Composable
private fun PermissionsCard() {
    val ctx = LocalContext.current
    val c = tb
    var tick by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { tick++ }
    val notifPerm = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { tick++ }

    @Suppress("UNUSED_EXPRESSION") tick
    val pm = ctx.getSystemService(PowerManager::class.java)
    val perms = buildList {
        add(
            Perm(
                stringResource(R.string.home_perm_notif_access), stringResource(R.string.home_perm_notif_access_desc),
                NotifListener.connected || enabledListener(ctx),
            ) { it.startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)) },
        )
        add(
            Perm(stringResource(R.string.home_perm_battery), stringResource(R.string.home_perm_battery_desc), pm.isIgnoringBatteryOptimizations(ctx.packageName)) {
                OemAutostart.openBatteryUnrestricted(it)
            },
        )
        if (Build.VERSION.SDK_INT >= 33) {
            add(
                Perm(
                    stringResource(R.string.home_perm_notif_post), stringResource(R.string.home_perm_notif_post_desc),
                    ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED,
                ) { notifPerm.launch(Manifest.permission.POST_NOTIFICATIONS) },
            )
        }
        OemAutostart.detect()?.let { oem ->
            // Üreticiye özgü arka plan / otomatik başlatma ayarı (durumu Android API ile okunamıyor: ok = null).
            val title = if (oem.kind == OemAutostart.Kind.SAMSUNG) {
                stringResource(R.string.home_perm_oem_samsung_title)
            } else {
                stringResource(R.string.home_perm_oem_autostart, oem.brandLabel)
            }
            val desc = if (oem.kind == OemAutostart.Kind.SAMSUNG) {
                stringResource(R.string.home_perm_oem_samsung_desc)
            } else {
                stringResource(R.string.home_perm_oem_autostart_desc)
            }
            add(Perm(title, desc, null) { OemAutostart.openSettings(it, oem) })
        }
    }
    val checkable = perms.filter { it.ok != null }
    val done = checkable.count { it.ok == true }
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SectionTitle(stringResource(R.string.home_setup_title), Modifier.weight(1f))
            Text("$done / ${checkable.size}", color = if (done == checkable.size) c.ok else c.goldHi, style = MaterialTheme.typography.labelMedium)
        }
        Spacer(Modifier.height(8.dp))
        GoldProgress(if (checkable.isEmpty()) 1f else done.toFloat() / checkable.size, active = false, tone = if (done == checkable.size) "ok" else "gold")
        Spacer(Modifier.height(6.dp))
        perms.forEach { p ->
            CheckRow(p.title, p.desc, p.ok == true) { p.action(ctx) }
        }
    }
}

@Composable
private fun Tip() {
    val c = tb
    Box(Modifier.padding(horizontal = 4.dp, vertical = 2.dp)) {
        Text(
            stringResource(R.string.home_tip, stringResource(R.string.widget_label), stringResource(R.string.tile_label)),
            color = c.faint,
            style = MaterialTheme.typography.bodySmall,
        )
    }
}

private fun enabledListener(ctx: Context): Boolean {
    val flat = Settings.Secure.getString(ctx.contentResolver, "enabled_notification_listeners") ?: return false
    return flat.split(':').any { ComponentName.unflattenFromString(it)?.packageName == ctx.packageName }
}
