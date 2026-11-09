package app.kopru.ui

import androidx.compose.foundation.Canvas
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
            Text("Ekosistem", Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Text("Bildirimler", color = c.muted, style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(end = 8.dp))
            Switch(s.notify, { Suite.setNotify(it) }, colors = SwitchDefaults.colors(checkedTrackColor = c.gold, checkedThumbColor = c.onGold))
        }

        val active = s.apps.count { it.running && it.id != "t-bridge" }
        StatusRow(
            ok = on,
            text = when {
                on && active > 0 -> "Bilgisayara bağlı · $active uygulama etkin"
                on -> "Bilgisayara bağlı · uygulamalar başlatılıyor"
                ui.conn == ConnState.CONNECTED -> "Bilgisayara bağlı · T-Bridge sürümü eski"
                else -> "Bilgisayara bağlı değil"
            },
        )

        s.message?.let { m ->
            Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.goldSoft).padding(14.dp)) {
                Text(m, color = c.goldHi, style = MaterialTheme.typography.bodyMedium)
            }
        }

        if (!on) {
            if (viewer.installed) TbCard { SectionTitle("Telefondaki uygulamalar"); Spacer8(); ViewerRow(viewer) }
            TbCard {
                Text(
                    if (ui.conn != ConnState.CONNECTED) "Bilgisayara bağlanınca T-Suite uygulamaları burada görünür." else "Bilgisayardaki T-Bridge sürümü ekosistemi desteklemiyor; güncelle.",
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
                if (ev.isEmpty()) Text(if (a.running) "Henüz olay yok" else "Kapalı", color = c.faint, style = MaterialTheme.typography.bodySmall)
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
        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(c.s2).padding(horizontal = 14.dp, vertical = 12.dp),
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
        AppState.MISSING -> Text("$name bu bilgisayarda kurulu değil", color = c.faint, style = MaterialTheme.typography.bodyMedium)
        AppState.STARTING -> Text("$name arka planda başlatılıyor…", color = c.muted, style = MaterialTheme.typography.bodyMedium)
        else -> {
            Text("$name otomatik başlatılamadı", color = c.muted, style = MaterialTheme.typography.bodyMedium)
            Spacer8()
            TbButton("Bilgisayarda aç", { Suite.launch(id) }, Modifier.fillMaxWidth())
        }
    }
}

@Composable
private fun DownloadsCard(active: List<DlItem>, done: List<DlDone>, st: AppState) {
    val c = tb
    TbCard {
        SectionTitle("İndirmeler")
        Spacer8()
        if (st != AppState.RUNNING && active.isEmpty() && done.isEmpty()) { OffState("T-Downloader", "t-downloader", st); return@TbCard }
        if (active.isEmpty()) Text("Etkin indirme yok", color = c.faint, style = MaterialTheme.typography.bodySmall)
        active.forEach { d ->
            Column(Modifier.padding(vertical = 6.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(d.name, Modifier.weight(1f), color = c.text, maxLines = 1, style = MaterialTheme.typography.bodyMedium)
                    Text(if (d.paused) "duraklatıldı" else "${(d.speed / 1048576).format1()} MB/sn", color = c.muted, style = MaterialTheme.typography.labelSmall)
                }
                Box(Modifier.padding(vertical = 5.dp).fillMaxWidth().height(6.dp).clip(RoundedCornerShape(3.dp)).background(c.s4)) {
                    Box(Modifier.fillMaxWidth((d.pct / 100.0).toFloat().coerceIn(0f, 1f)).height(6.dp).background(c.goldBrush))
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("%${d.pct.toInt()}" + (if (d.etaSec >= 0) " · ${eta(d.etaSec)}" else ""), Modifier.weight(1f), color = c.faint, style = MaterialTheme.typography.labelSmall)
                    Chip(if (d.paused) "Devam" else "Duraklat") { Suite.send("t-downloader", if (d.paused) "download.resume" else "download.pause", """{"id":"${d.id}"}""") }
                }
            }
        }
        if (done.isNotEmpty()) {
            Spacer8()
            Text("Tamamlananlar", color = c.muted, style = MaterialTheme.typography.labelLarge)
            done.forEach { d ->
                Row(Modifier.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(d.name, Modifier.weight(1f), color = c.text, maxLines = 1, style = MaterialTheme.typography.bodySmall)
                    Chip("Telefona al") { Suite.send("t-bridge", "file.send", org.json.JSONObject().put("path", d.path).toString()) { ok, _, e -> Suite.toast(if (ok) "Gönderiliyor: ${d.name}" else (e ?: "Gönderilemedi")) } }
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
                Text("Bellek temizleme", color = c.text, style = MaterialTheme.typography.titleMedium)
                Text("Akıllı: Arka plandaki uygulamaların gereksiz belleğini boşaltır, açık işinize dokunmaz (önerilen).", color = c.muted, style = MaterialTheme.typography.bodyMedium)
                Text("Tam: Önbellek dahil her şeyi boşaltır; yönetici izni gerekir, kısa süre yavaşlık olabilir.", color = c.muted, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f)) { SectionTitle("RAM") }
            Box(
                Modifier.size(28.dp).clip(RoundedCornerShape(14.dp)).background(c.s4).clickable { help = true },
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
                Text(if (ram != null) "${Suite.fmtMb(ram.usedMb.toDouble())} / ${Suite.fmtMb(ram.totalMb.toDouble())}" else "Veri bekleniyor…", color = c.text, style = MaterialTheme.typography.bodyMedium)
                Spacer8()
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TbButton("Akıllı", { Suite.ramClean(false) }, Modifier.weight(1f))
                    TbButton("Tam", { Suite.ramClean(true) }, Modifier.weight(1f))
                }
            }
        }
    }
}

@Composable
private fun DiskCard(disks: List<app.kopru.DiskRow>, st: AppState) {
    val c = tb
    TbCard {
        SectionTitle("Disk")
        Spacer8()
        if (st != AppState.RUNNING && disks.isEmpty()) { OffState("T-DeskTree", "t-desktree", st); return@TbCard }
        if (disks.isEmpty()) Text("Okunuyor…", color = c.faint, style = MaterialTheme.typography.bodySmall)
        disks.forEach { d ->
            Column(Modifier.padding(vertical = 5.dp)) {
                Row {
                    Text(d.drive, Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium)
                    Text("${Suite.fmtMb(d.freeBytes / 1048576.0)} boş", color = if (d.usedPct > 0.9f) Color(0xFFF87171) else c.muted, style = MaterialTheme.typography.labelSmall)
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
        SectionTitle("Uygulamalar")
        Spacer8()
        if (s.apps.isEmpty()) Text("Bu bilgisayarda kurulu T-Suite uygulaması bulunamadı", color = c.faint, style = MaterialTheme.typography.bodySmall)
        s.apps.forEach { a ->
            Row(Modifier.padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(if (a.running) c.goldHi else c.faint))
                Text(a.name.ifEmpty { a.id }, Modifier.padding(start = 10.dp).weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium)
                Text("v${a.version}", color = c.muted, style = MaterialTheme.typography.labelSmall)
                if (a.updateAvailable != null) Box(Modifier.padding(start = 8.dp)) { Pill("güncelleme", "gold") }
            }
        }
        if (viewer.installed) {
            Spacer8()
            Text("Bu telefonda", color = c.muted, style = MaterialTheme.typography.labelLarge)
            ViewerRow(viewer)
        }
        val n = s.apps.count { it.updateAvailable != null }
        Spacer8()
        TbButton(if (n > 0) "Hepsini güncelle ($n)" else "Hepsini güncelle", { Suite.updateAll() }, Modifier.fillMaxWidth(), enabled = n > 0)
    }
}

private fun Double.format1() = String.format(java.util.Locale.getDefault(), "%.1f", this)
private fun eta(sec: Long) = if (sec >= 3600) "${sec / 3600} sa ${sec % 3600 / 60} dk" else if (sec >= 60) "${sec / 60} dk ${sec % 60} sn" else "$sec sn"

/** T-Desk Viewer satırı: sürüm (PackageManager), "Aç" ve güncelleme varsa altın rozet (viewer'ın güncelleme ekranına derin bağlantı). */
@Composable
private fun ViewerRow(v: app.kopru.ViewerState) {
    val c = tb
    val ctx = LocalContext.current
    Row(Modifier.padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(c.goldHi))
        Text("T-Desk Viewer", Modifier.padding(start = 10.dp).weight(1f), color = c.text, style = MaterialTheme.typography.bodyMedium, maxLines = 1)
        Text("v${v.version}", color = c.muted, style = MaterialTheme.typography.labelSmall)
        if (v.update != null) {
            Box(Modifier.padding(start = 8.dp).clip(RoundedCornerShape(8.dp)).clickable { ViewerLink.openUpdateScreen(ctx) }) { Pill("Güncelleme var", "gold") }
        }
        Box(Modifier.padding(start = 8.dp)) { Chip("Aç") { if (!ViewerLink.launchViewer(ctx)) Suite.toast("Uygulama açılamadı") } }
    }
}
