package app.kopru.ui

import android.Manifest
import android.content.ActivityNotFoundException
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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
        ui.startError?.let { ErrorCard("Çekirdek başlatılamadı: $it") }
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
                Toast.makeText(ctx, "Bu QR kod T-Bridge'e ait değil.", Toast.LENGTH_LONG).show()
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
            confirmButton = { TextButton({ Core.clearPairError() }) { Text("Tamam") } },
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
        ConnState.CONNECTED -> Triple("Bağlı", ui.peer?.name ?: "Bilgisayar", "Pano, dosya, bildirim ve kumanda hazır.")
        ConnState.CONNECTING -> Triple("Durum", "Bağlanıyor…", "Bilgisayar aranıyor.")
        ConnState.DISCONNECTED ->
            if (ui.peers.isEmpty()) Triple("Hoş geldin", "Bilgisayarını eşleştir", "Telefon ve bilgisayar aynı Wi‑Fi'da olmalı.")
            else Triple("Durum", "Bilgisayar bekleniyor", "T-Bridge bilgisayarda açılınca kendiliğinden bağlanır.")
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
        ActionTile("Panoyu\ngönder", Icons.Outlined.ContentPaste, Modifier.weight(1f)) {
            val cm = ctx.getSystemService(ClipboardManager::class.java)
            val text = cm?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(ctx)?.toString()
            val msg = when {
                text.isNullOrEmpty() -> "Panoda metin yok"
                Core.sendClipboardText(text) -> "PC panosuna gönderildi"
                else -> "Zaten gönderildi"
            }
            Toast.makeText(ctx, msg, Toast.LENGTH_SHORT).show()
        }
        ActionTile("Dosya\ngönder", Icons.Outlined.Upload, Modifier.weight(1f)) { go(1) }
        ActionTile("PC'yi\nkilitle", Icons.Outlined.Lock, Modifier.weight(1f)) {
            Core.node?.sendPc(PcCommand.Power(PowerAction.LOCK))
            Toast.makeText(ctx, "Bilgisayar kilitlendi", Toast.LENGTH_SHORT).show()
        }
    }
}

@Composable
private fun ActionTile(label: String, icon: ImageVector, modifier: Modifier, onClick: () -> Unit) {
    val c = tb
    val shape = RoundedCornerShape(18.dp)
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
private fun explainPairError(raw: String): Pair<String, List<String>> {
    val r = raw.lowercase()
    return when {
        "kod" in r || "reddedildi" in r -> "Kod kabul edilmedi" to listOf(
            "Kodu yanlış girmiş olabilirsin ya da süresi dolmuştur (5 dakika).",
            "Bilgisayardaki güncel kodu gir veya QR'ı yeniden okut.",
        )
        "sürüm" in r -> "Sürümler uyuşmuyor" to listOf("İki cihazda da T-Bridge'i güncelle: Ayarlar → Şimdi denetle.")
        "certificate" in r || "sertifika" in r || "tls" in r || "fingerprint" in r -> "Bilgisayar doğrulanamadı" to listOf(
            "Bu QR eski bir kuruluma ait olabilir.",
            "Bilgisayardaki güncel QR'ı yeniden okut.",
        )
        else -> "Bilgisayara ulaşılamadı" to listOf(
            "Bilgisayarda T-Bridge açık mı?",
            "İki cihaz da aynı Wi‑Fi'da mı? Mobil veriyi ve VPN'i kapat.",
            "Bilgisayarda T-Bridge → Ayarlar → Güvenlik: güvenlik duvarı izni verilmiş olmalı.",
            "Misafir ağındaysan veya modemde istemci izolasyonu açıksa cihazlar birbirini göremez.",
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
            Text("Bağlanamıyor musun?", Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.titleSmall)
            Text(if (open) "Kapat" else "Çözümler", color = c.goldHi, style = MaterialTheme.typography.labelLarge)
        }
        if (open) {
            Spacer(Modifier.height(10.dp))
            listOf(
                "Bilgisayarda T-Bridge açık olmalı.",
                "İki cihaz aynı Wi‑Fi'da olmalı; mobil veriyi ve VPN'i kapat.",
                "Bilgisayarda T-Bridge → Ayarlar → Güvenlik: güvenlik duvarı izni verilmiş olmalı.",
                "Bilgisayar listede çıkmıyorsa Yeniden tara'ya bas ya da Wi‑Fi'ı kapatıp aç.",
                "Misafir ağları ve modemdeki istemci izolasyonu cihazların birbirini görmesini engeller.",
            ).forEachIndexed { i, s ->
                Row(Modifier.padding(vertical = 4.dp)) {
                    Text("${i + 1}.", Modifier.width(22.dp), color = c.gold, style = MaterialTheme.typography.bodyMedium)
                    Text(s, color = c.text2, style = MaterialTheme.typography.bodyMedium)
                }
            }
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                TbButton("Yeniden tara", {
                    Core.node?.rescan()
                    Toast.makeText(ctx, "Ağ yeniden taranıyor", Toast.LENGTH_SHORT).show()
                }, Modifier.weight(1f), height = 44.dp)
                TbButton("Wi‑Fi ayarları", {
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
    TbCard(gold = true) {
        SectionTitle("Eşleştirme", color = c.gold)
        Spacer(Modifier.height(4.dp))
        Text("Bilgisayarı bağla", color = c.text, style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(4.dp))
        Text("Bilgisayarda T-Bridge'i aç; ekranda bir QR kod ve 6 haneli kod görünür.", color = c.muted, style = MaterialTheme.typography.bodyMedium)
        Spacer(Modifier.height(14.dp))
        GoldButton("QR ile eşleştir", onScan, Modifier.fillMaxWidth(), icon = Icons.Outlined.QrCodeScanner)
        Spacer(Modifier.height(16.dp))
        SectionTitle("veya ağda bulunanlardan seç")
        Spacer(Modifier.height(6.dp))
        val list = ui.discovered.filter { !it.paired }
        if (list.isEmpty()) {
            Row(Modifier.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(Modifier.size(16.dp), color = c.gold, strokeWidth = 2.dp)
                Spacer(Modifier.width(10.dp))
                Text("Aranıyor…", color = c.muted, style = MaterialTheme.typography.bodyMedium)
            }
        }
        list.forEach { s ->
            Row(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).clickable { onPick(s) }.padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconTile(Icons.Outlined.Computer, size = 38.dp)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(s.name, color = c.text, style = MaterialTheme.typography.titleSmall)
                    Text("Aynı ağda", color = c.muted, style = MaterialTheme.typography.bodySmall)
                }
                Text("Kodu gir", color = c.goldHi, style = MaterialTheme.typography.labelLarge)
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
            SectionTitle("Eşleşmiş bilgisayarlar", Modifier.weight(1f))
            TextButton(onNew) { Text("Yeni ekle", color = c.goldHi) }
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
                            Pill("bağlı")
                        }
                    }
                    Text(if (live) "Şifreli bağlantı" else "Eşleşti", color = c.muted, style = MaterialTheme.typography.bodySmall)
                }
                TextButton({ confirm = p.id }) { Text("Kaldır", color = c.muted) }
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
                }) { Text("Kaldır", color = c.bad) }
            },
            dismissButton = { TextButton({ confirm = null }) { Text("Vazgeç", color = c.text2) } },
            title = { Text("Eşleşme kaldırılsın mı?") },
            text = { Text("Tekrar bağlanmak için yeniden eşleştirmen gerekir.", color = c.text2) },
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
        title = { Text("${srv.name} ile eşleş") },
        text = {
            Column {
                Text("Bilgisayar ekranındaki 6 haneli kodu gir.", color = c.text2)
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
        confirmButton = { TextButton({ onOk(code) }, enabled = code.length == 6) { Text("Eşleştir", color = if (code.length == 6) c.goldHi else c.faint) } },
        dismissButton = { TextButton(onDismiss) { Text("Vazgeç", color = c.text2) } },
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
                "Bildirim erişimi", "Bildirimleri PC'de göstermek ve medyayı kontrol etmek için.",
                NotifListener.connected || enabledListener(ctx),
            ) { it.startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)) },
        )
        add(
            Perm("Pil kısıtlaması yok", "Ekran kapalıyken bağlantı kopmasın.", pm.isIgnoringBatteryOptimizations(ctx.packageName)) {
                @Suppress("BatteryLife")
                it.startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${it.packageName}")))
            },
        )
        if (Build.VERSION.SDK_INT >= 33) {
            add(
                Perm(
                    "Bildirim izni", "Durum ve 'dosya alındı' bildirimleri için.",
                    ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED,
                ) { notifPerm.launch(Manifest.permission.POST_NOTIFICATIONS) },
            )
        }
        if (Build.MANUFACTURER.equals("Xiaomi", true) || Build.MANUFACTURER.equals("Redmi", true) || Build.MANUFACTURER.equals("POCO", true)) {
            // MIUI bu ayarın durumunu uygulamalara bildirmez: ok = null (doğrulanamıyor).
            add(Perm("MIUI: Otomatik başlatma", "Durumu okunamıyor; açık olduğundan emin ol.", null) { openMiuiAutostart(it) })
        }
    }
    val checkable = perms.filter { it.ok != null }
    val done = checkable.count { it.ok == true }
    TbCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            SectionTitle("Kurulum", Modifier.weight(1f))
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
            "İpucu: Ana ekrana “T-Bridge: Panoyu gönder” widget'ını ya da hızlı ayarlara “Panoyu PC'ye” kutucuğunu ekleyerek kopyaladığını tek dokunuşla gönderebilirsin.",
            color = c.faint,
            style = MaterialTheme.typography.bodySmall,
        )
    }
}

private fun enabledListener(ctx: Context): Boolean {
    val flat = Settings.Secure.getString(ctx.contentResolver, "enabled_notification_listeners") ?: return false
    return flat.split(':').any { ComponentName.unflattenFromString(it)?.packageName == ctx.packageName }
}

private fun openMiuiAutostart(ctx: Context) {
    val intents = listOf(
        Intent().setComponent(ComponentName("com.miui.securitycenter", "com.miui.permcenter.autostart.AutoStartManagementActivity")),
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}")),
    )
    for (i in intents) {
        try {
            ctx.startActivity(i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return
        } catch (_: ActivityNotFoundException) {
        } catch (_: SecurityException) {
        }
    }
}
