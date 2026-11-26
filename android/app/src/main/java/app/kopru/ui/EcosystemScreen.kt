package app.kopru.ui

import androidx.compose.foundation.Canvas
import app.kopru.R
import androidx.annotation.StringRes
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.ViewerLink
import app.kopru.Core
import app.kopru.DlDone
import app.kopru.DlItem
import app.kopru.Suite
import app.kopru.UiState
import kotlinx.coroutines.delay
import uniffi.kopru_core.ConnState

private val KNOWN = setOf("t-ramcash", "t-downloader", "t-desktree", "t-bridge")

@Composable
fun EcosystemScreen(ui: UiState) {
    val c = tb
    val s by Suite.ui.collectAsStateWithLifecycle()
    val on = ui.conn == ConnState.CONNECTED && Suite.supported()
    fun running(id: String) = s.apps.any { it.id == id && it.running }

    val ctx = LocalContext.current
    val viewer by ViewerLink.state.collectAsStateWithLifecycle()
    val owner = LocalLifecycleOwner.current
    DisposableEffect(owner) {
        val obs = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) ViewerLink.refresh(ctx) }
        owner.lifecycle.addObserver(obs)
        onDispose { owner.lifecycle.removeObserver(obs) }
    }

    LaunchedEffect(on, running("t-desktree")) {
        while (on && running("t-desktree")) { Suite.refreshDisks(); delay(60_000) }
    }
    // Otomatik başlatma ~75 sn içinde tutmadıysa (uygulama hâlâ kapalı) elle "Aç" yedeği gösterilir.
    var grace by remember { mutableStateOf(false) }
    LaunchedEffect(on) {
        grace = false
        if (on) { delay(75_000); grace = true }
    }
    LaunchedEffect(s.message) {
        if (s.message != null) { delay(3500); Suite.clearMessage() }
    }

    Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.eco_title), Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Text(stringResource(R.string.eco_notifications), color = c.muted, style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(end = 8.dp))
            Switch(s.notify, { Suite.setNotify(it) }, colors = SwitchDefaults.colors(checkedTrackColor = c.gold, checkedThumbColor = c.onGold))
        }

        val active = s.apps.count { it.running && it.id != "t-bridge" }
        StatusRow(
            ok = on,
            text = when {
                on && active > 0 -> pluralStringResource(R.plurals.eco_status_active, active, active)
                on -> stringResource(R.string.eco_status_starting)
                ui.conn == ConnState.CONNECTED -> stringResource(R.string.eco_status_old)
                else -> stringResource(R.string.eco_status_off)
            },
        )

        s.message?.let { m ->
            Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(c.goldSoft).padding(14.dp)) {
                Text(m, color = c.goldHi, style = MaterialTheme.typography.bodyMedium)
            }
        }

        if (!on) {
            if (viewer.installed) TbCard { SectionTitle(stringResource(R.string.eco_phone_apps)); Spacer8(); ViewerRow(viewer) }
            TbCard {
                Text(
                    if (ui.conn != ConnState.CONNECTED) stringResource(R.string.eco_wait_pc) else stringResource(R.string.eco_pc_old),
                    color = c.muted, style = MaterialTheme.typography.bodyMedium,
                )
            }
            return@Column
        }

        DownloadsCard(s.downloads.values.toList(), s.completed, appState(s, "t-downloader", grace))
        RamCard(s.ram, appState(s, "t-ramcash", grace))
        DiskCard(s.disks, appState(s, "t-desktree", grace))
        AppsCard(s, viewer)
        s.apps.filter { it.id !in KNOWN }.forEach { a ->
            TbCard {
                SectionTitle(a.name.ifEmpty { a.id })
                Spacer8()
                val ev = s.events[a.id].orEmpty()
                if (ev.isEmpty()) Text(if (a.running) stringResource(R.string.eco_no_events) else stringResource(R.string.eco_closed), color = c.faint, style = MaterialTheme.typography.bodySmall)
                ev.forEach { Text(it, color = c.muted, style = MaterialTheme.typography.bodySmall, maxLines = 1) }
            }
        }
    }
}

@Composable
private fun Spacer8() = Box(Modifier.height(8.dp))

/** Uygulamanın PC'deki durumu: kurulu değil / başlatılıyor / başlatılamadı (yedek "Aç") / çalışıyor. */
private enum class AppState { RUNNING, STARTING, FAILED, MISSING }

private fun appState(s: app.kopru.SuiteUi, id: String, grace: Boolean): AppState {
    val a = s.apps.firstOrNull { it.id == id } ?: return AppState.MISSING
    return if (a.running) AppState.RUNNING else if (grace) AppState.FAILED else AppState.STARTING
}

@Composable
private fun StatusRow(ok: Boolean, text: String) {
    val c = tb
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(c.s2).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(10.dp).clip(RoundedCornerShape(5.dp)).background(if (ok) c.goldHi else c.faint))
        Text(text, Modifier.padding(start = 12.dp), color = if (ok) c.text else c.muted, style = MaterialTheme.typography.bodyMedium)
    }
}

/** Kapalı kart durumu: net metin; elle "Aç" yalnızca otomatik başlatma tutmadıysa. */
@Composable
private fun OffState(name: String, id: String, st: AppState) {
    val c = tb
    when (st) {
        AppState.MISSING -> {
            // Kart yerinde kalır; × yalnızca bu daveti kapatır (kalıcı gizleme yok).
            var closed by remember { mutableStateOf(false) }
            val ctx = LocalContext.current
            val inv = inviteFor(id)
            if (closed || inv == null) Text(stringResource(R.string.eco_not_installed, name), color = c.faint, style = MaterialTheme.typography.bodyMedium)
            else InviteCard(inv.app, stringResource(inv.title), stringResource(inv.body), stringResource(R.string.inv_pc_cta), { shareInviteLink(ctx, inv.app) }, { closed = true })
        }
        AppState.STARTING -> Text(stringResource(R.string.eco_off_starting, name), color = c.muted, style = MaterialTheme.typography.bodyMedium)
        else -> {
            Text(stringResource(R.string.eco_off_failed, name), color = c.muted, style = MaterialTheme.typography.bodyMedium)
            Spacer8()
            TbButton(stringResource(R.string.eco_open_pc), { Suite.launch(id) }, Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun DownloadsCard(active: List<DlItem>, done: List<DlDone>, st: AppState) {
    val c = tb
    val sendingTpl = stringResource(R.string.eco_queued, "%s")
    val sendFailed = stringResource(R.string.eco_send_failed)
    val controlFailed = stringResource(R.string.suite_download_control_failed)
    val waiting = stringResource(R.string.eco_action_waiting)
    val pending = remember { mutableStateMapOf<String, Boolean>() }
    val handler = remember { android.os.Handler(android.os.Looper.getMainLooper()) }
    TbCard {
        SectionTitle(stringResource(R.string.eco_downloads))
        Spacer8()
        if (st != AppState.RUNNING && active.isEmpty() && done.isEmpty()) { OffState("T-Downloader", "t-downloader", st); return@TbCard }
        if (active.isEmpty()) Text(stringResource(R.string.eco_no_active_dl), color = c.faint, style = MaterialTheme.typography.bodySmall)
        active.forEach { d ->
            Column(Modifier.padding(vertical = 6.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(d.name, Modifier.weight(1f), color = c.text, maxLines = 1, style = MaterialTheme.typography.bodyMedium)
                    Text(if (d.paused) stringResource(R.string.eco_paused) else stringResource(R.string.eco_speed, (d.speed / 1048576).format1()), color = c.muted, style = MaterialTheme.typography.labelSmall)
                }
                Box(Modifier.padding(vertical = 5.dp).fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(c.s4)) {
                    Box(Modifier.fillMaxWidth((d.pct / 100.0).toFloat().coerceIn(0f, 1f)).height(6.dp).background(c.goldBrush))
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("%${d.pct.toInt()}" + (if (d.etaSec >= 0) " · ${eta(d.etaSec)}" else ""), Modifier.weight(1f), color = c.faint, style = MaterialTheme.typography.labelSmall)
                    Chip(if (pending[d.id] == true) waiting else if (d.paused) stringResource(R.string.eco_resume) else stringResource(R.string.eco_pause), enabled = pending[d.id] != true) {
                        pending[d.id] = true
                        Suite.send("t-downloader", if (d.paused) "download.resume" else "download.pause", org.json.JSONObject().put("id", d.id).toString()) { ok, _, error ->
                            handler.post { pending.remove(d.id); if (!ok) Suite.toast(error ?: controlFailed) }
                        }
                    }
                }
            }
        }
        if (done.isNotEmpty()) {
            Spacer8()
            Text(stringResource(R.string.eco_completed), color = c.muted, style = MaterialTheme.typography.labelLarge)
            done.forEach { d ->
                Row(Modifier.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(d.name, Modifier.weight(1f), color = c.text, maxLines = 1, style = MaterialTheme.typography.bodySmall)
                    Chip(if (pending[d.id] == true) waiting else stringResource(R.string.eco_get_on_phone), enabled = pending[d.id] != true) {
                        pending[d.id] = true
                        Suite.send("t-bridge", "download.send", org.json.JSONObject().put("id", d.id).toString()) { ok, result, e ->
                            val accepted = ok && (result?.optJSONArray("ids")?.length() ?: 0) > 0
                            handler.post { pending.remove(d.id); Suite.toast(if (accepted) sendingTpl.format(d.name) else (e ?: sendFailed)) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
private fun RamCard(ram: app.kopru.RamInfo?, st: AppState) {
    val c = tb
    var help by remember { mutableStateOf(false) }
    if (help) {
        ModalBottomSheet(onDismissRequest = { help = false }, sheetState = rememberModalBottomSheetState(), containerColor = c.s1) {
            Column(Modifier.padding(horizontal = 22.dp).padding(bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Text(stringResource(R.string.eco_ram_help_title), color = c.text, style = MaterialTheme.typography.titleMedium)
                Text(stringResource(R.string.eco_ram_help_smart), color = c.muted, style = MaterialTheme.typography.bodyMedium)
                Text(stringResource(R.string.eco_ram_help_full), color = c.muted, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f)) { SectionTitle(stringResource(R.string.eco_ram)) }
            Box(
                Modifier.size(28.dp).clip(RoundedCornerShape(12.dp)).background(c.s4).clickable { help = true },
                contentAlignment = Alignment.Center,
            ) { Text("?", color = c.goldHi, style = MaterialTheme.typography.labelLarge) }
        }
        Spacer8()
        if (st != AppState.RUNNING && ram == null) { OffState("T-RamCash", "t-ramcash", st); return@TbCard }
        Row(verticalAlignment = Alignment.CenterVertically) {
            val pct = (ram?.usedPct ?: 0) / 100f
            Box(Modifier.size(96.dp), contentAlignment = Alignment.Center) {
                Canvas(Modifier.size(96.dp)) {
                    val st = Stroke(10.dp.toPx(), cap = StrokeCap.Round)
                    drawArc(c.s4, 135f, 270f, false, Offset(5.dp.toPx(), 5.dp.toPx()), Size(size.width - 10.dp.toPx(), size.height - 10.dp.toPx()), style = st)
                    drawArc(c.goldBrush, 135f, 270f * pct, false, Offset(5.dp.toPx(), 5.dp.toPx()), Size(size.width - 10.dp.toPx(), size.height - 10.dp.toPx()), style = st)
                }
                Text("%${ram?.usedPct ?: "—"}", color = c.text, style = MaterialTheme.typography.titleMedium)
            }
            Column(Modifier.padding(start = 16.dp).weight(1f)) {
                Text(if (ram != null) "${Suite.fmtMb(ram.usedMb.toDouble())} / ${Suite.fmtMb(ram.totalMb.toDouble())}" else stringResource(R.string.eco_waiting_data), color = c.text, style = MaterialTheme.typography.bodyMedium)
                Spacer8()
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TbButton(stringResource(R.string.eco_smart), { Suite.ramClean(false) }, Modifier.weight(1f))
                    TbButton(stringResource(R.string.eco_full), { Suite.ramClean(true) }, Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun DiskCard(disks: List<app.kopru.DiskRow>, st: AppState) {
    val c = tb
    TbCard {
        SectionTitle(stringResource(R.string.eco_disk))
        Spacer8()
        if (st != AppState.RUNNING && disks.isEmpty()) { OffState("T-DeskTree", "t-desktree", st); return@TbCard }
        if (disks.isEmpty()) Text(stringResource(R.string.eco_reading), color = c.faint, style = MaterialTheme.typography.bodySmall)
        disks.forEach { d ->
            Column(Modifier.padding(vertical = 5.dp)) {
                Row {
                    Text(d.drive, Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium)
                    Text(stringResource(R.string.eco_free, Suite.fmtMb(d.freeBytes / 1048576.0)), color = if (d.usedPct > 0.9f) Color(0xFFF87171) else c.muted, style = MaterialTheme.typography.labelSmall)
                }
                Box(Modifier.padding(top = 4.dp).fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(c.s4)) {
                    Box(Modifier.fillMaxWidth(d.usedPct).height(6.dp).background(c.goldBrush))
                }
            }
        }
    }
}

@Composable
private fun AppsCard(s: app.kopru.SuiteUi, viewer: app.kopru.ViewerState) {
    val c = tb
    TbCard {
        SectionTitle(stringResource(R.string.eco_apps))
        Spacer8()
        if (s.apps.isEmpty()) Text(stringResource(R.string.eco_no_pc_apps), color = c.faint, style = MaterialTheme.typography.bodySmall)
        s.apps.forEach { a ->
            Row(Modifier.padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(if (a.running) c.goldHi else c.faint))
                Text(a.name.ifEmpty { a.id }, Modifier.padding(start = 10.dp).weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium)
                Text("v${a.version}", color = c.muted, style = MaterialTheme.typography.labelSmall)
                if (a.updateAvailable != null) Box(Modifier.padding(start = 8.dp)) { Pill(stringResource(R.string.eco_update_badge), "gold") }
            }
        }
        if (viewer.installed) {
            Spacer8()
            Text(stringResource(R.string.eco_on_phone), color = c.muted, style = MaterialTheme.typography.labelLarge)
            ViewerRow(viewer)
        } else {
            // T-Desk Viewer yoksa satır yerinde kalır, içinde davet görünür (× yalnızca bu daveti kapatır).
            var closed by remember { mutableStateOf(false) }
            val ictx = LocalContext.current
            if (!closed) {
                Spacer8()
                InviteCard(InviteApp.VIEWER, stringResource(R.string.inv_viewer_title), stringResource(R.string.inv_viewer_body), stringResource(R.string.inv_viewer_cta), { openInviteUrl(ictx, InviteApp.VIEWER) }, { closed = true })
            }
        }
        val n = s.apps.count { it.updateAvailable != null }
        Spacer8()
        TbButton(if (s.updating) stringResource(R.string.eco_action_waiting) else if (n > 0) stringResource(R.string.eco_update_all_n, n) else stringResource(R.string.eco_update_all), { Suite.updateAll() }, Modifier.fillMaxWidth(), enabled = n > 0 && !s.updating)
    }
}

private fun Double.format1() = String.format(java.util.Locale.getDefault(), "%.1f", this)
@Composable
private fun eta(sec: Long): String =
    if (sec >= 3600) stringResource(R.string.eco_eta_h, sec / 3600, sec % 3600 / 60)
    else if (sec >= 60) stringResource(R.string.eco_eta_m, sec / 60, sec % 60)
    else stringResource(R.string.eco_eta_s, sec)

private class InviteSpec(val app: InviteApp, @StringRes val title: Int, @StringRes val body: Int)

/** Ekosistem kartındaki PC uygulaması için bağlama özel davet metni. */
private fun inviteFor(id: String): InviteSpec? = when (id) {
    "t-downloader" -> InviteSpec(InviteApp.DOWNLOADER, R.string.inv_dl_title, R.string.inv_dl_body)
    "t-ramcash" -> InviteSpec(InviteApp.RAMCASH, R.string.inv_ram_title, R.string.inv_ram_body)
    "t-desktree" -> InviteSpec(InviteApp.DESKTREE, R.string.inv_disk_title, R.string.inv_disk_body)
    else -> null
}

/** T-Desk Viewer satırı: sürüm (PackageManager), "Aç" ve güncelleme varsa altın rozet (viewer'ın güncelleme ekranına derin bağlantı). */
@Composable
private fun ViewerRow(v: app.kopru.ViewerState) {
    val c = tb
    val ctx = LocalContext.current
    val openFailed = stringResource(R.string.eco_open_failed)
    Row(Modifier.padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(c.goldHi))
        Text("T-Desk Viewer", Modifier.padding(start = 10.dp).weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium, maxLines = 1)
        Text("v${v.version}", color = c.muted, style = MaterialTheme.typography.labelSmall)
        if (v.update != null) {
            Box(Modifier.padding(start = 8.dp).clip(RoundedCornerShape(8.dp)).clickable { ViewerLink.openUpdateScreen(ctx) }) { Pill(stringResource(R.string.eco_update_available), "gold") }
        }
        Box(Modifier.padding(start = 8.dp)) { Chip(stringResource(R.string.eco_open)) { if (!ViewerLink.launchViewer(ctx)) Suite.toast(openFailed) } }
    }
}
