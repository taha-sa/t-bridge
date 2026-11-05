package app.kopru.ui

import android.Manifest
import android.content.ActivityNotFoundException
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
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.Computer
import androidx.compose.material.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import app.kopru.Core
import app.kopru.NotifListener
import app.kopru.UiState
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.ServerInfo

@Composable
fun HomeScreen(ui: UiState) {
    val ctx = LocalContext.current
    var pairing by remember { mutableStateOf(false) }
    var codeFor by remember { mutableStateOf<ServerInfo?>(null) }
    val showPair = pairing || ui.peers.isEmpty()

    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        StatusCard(ui)

        ui.startError?.let { ErrorCard("Çekirdek başlatılamadı: $it") }

        if (showPair && ui.conn != ConnState.CONNECTED) {
            PairCard(ui, onScan = { scanQr(ctx) }, onPick = { codeFor = it })
        } else {
            PeersCard(ui, onNew = { pairing = true })
        }
        if (pairing && ui.conn == ConnState.CONNECTED) pairing = false

        PermissionsCard()

        Text(
            "İpucu: Ana ekrana “T-Bridge: Panoyu gönder” widget'ını ya da hızlı ayarlara “Panoyu PC'ye” kutucuğunu ekleyerek kopyaladığını tek dokunuşla gönderebilirsin.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
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
            confirmButton = { TextButton({ Core.clearPairError() }) { Text("Tamam") } },
            title = { Text("Eşleştirilemedi") },
            text = { Text(err) },
        )
    }
}

private fun scanQr(ctx: Context) {
    val opts = GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).enableAutoZoom().build()
    GmsBarcodeScanning.getClient(ctx, opts).startScan()
        .addOnSuccessListener { code ->
            val raw = code.rawValue.orEmpty()
            runCatching { Core.node?.pairUri(raw) }.onFailure {
                Toast.makeText(ctx, "Geçersiz QR: ${it.message}", Toast.LENGTH_LONG).show()
            }
        }
        .addOnFailureListener {
            Toast.makeText(ctx, "QR okuyucu açılamadı; listeden seçip kodu gir.", Toast.LENGTH_LONG).show()
        }
}

@Composable
private fun StatusCard(ui: UiState) {
    val (color, title) = when (ui.conn) {
        ConnState.CONNECTED -> Color(0xFF16A34A) to "${ui.peer?.name ?: "Bilgisayar"} bağlı"
        ConnState.CONNECTING -> Color(0xFFD97706) to "Bağlanıyor…"
        ConnState.DISCONNECTED -> MaterialTheme.colorScheme.outline to if (ui.peers.isEmpty()) "Eşleşmiş bilgisayar yok" else "Bilgisayar bekleniyor"
    }
    Card(Modifier.fillMaxWidth()) {
        Row(Modifier.padding(18.dp), verticalAlignment = Alignment.CenterVertically) {
            Surface(Modifier.size(12.dp), shape = CircleShape, color = color) {}
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
                Text(
                    if (ui.conn == ConnState.CONNECTED) "Pano, dosya, bildirim ve kumanda hazır." else "Telefon ve bilgisayar aynı Wi-Fi'da olmalı.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (ui.conn == ConnState.CONNECTING) CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
        }
    }
}

@Composable
private fun ErrorCard(msg: String) {
    Card(Modifier.fillMaxWidth()) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.WarningAmber, null, tint = MaterialTheme.colorScheme.error)
            Spacer(Modifier.width(10.dp))
            Text(msg, color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
private fun PairCard(ui: UiState, onScan: () -> Unit, onPick: (ServerInfo) -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("Bilgisayarı eşleştir", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Text(
                "Bilgisayarda T-Bridge'i aç; ekranda bir QR kod ve 6 haneli kod görünecek.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Button(onScan, Modifier.fillMaxWidth()) {
                Icon(Icons.Outlined.QrCodeScanner, null)
                Spacer(Modifier.width(8.dp))
                Text("QR ile eşleştir")
            }
            Text("veya ağda bulunanlardan seç:", style = MaterialTheme.typography.labelLarge)
            val list = ui.discovered.filter { !it.paired }
            if (list.isEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(10.dp))
                    Text("Aranıyor…", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            list.forEach { s ->
                Row(
                    Modifier.fillMaxWidth().clickable { onPick(s) }.padding(vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Outlined.Computer, null)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(s.name, fontWeight = FontWeight.Medium)
                        Text(s.addrs.firstOrNull().orEmpty(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Text("Kodu gir", color = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }
}

@Composable
private fun PeersCard(ui: UiState, onNew: () -> Unit) {
    var confirm by remember { mutableStateOf<String?>(null) }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(18.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Eşleşmiş bilgisayarlar", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                TextButton(onNew) { Text("Yeni ekle") }
            }
            ui.peers.forEach { p ->
                Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Outlined.Computer, null)
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(p.name, fontWeight = FontWeight.Medium)
                        Text(
                            if (ui.peer?.id == p.id) "Bağlı · ${p.addrs.firstOrNull().orEmpty()}" else p.addrs.firstOrNull().orEmpty(),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    TextButton({ confirm = p.id }) { Text("Kaldır") }
                }
            }
        }
    }
    confirm?.let { id ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            confirmButton = {
                TextButton({
                    Core.node?.unpair(id)
                    Core.refreshPeers()
                    confirm = null
                }) { Text("Kaldır") }
            },
            dismissButton = { TextButton({ confirm = null }) { Text("Vazgeç") } },
            title = { Text("Eşleşme kaldırılsın mı?") },
            text = { Text("Tekrar bağlanmak için yeniden eşleştirmen gerekir.") },
        )
    }
}

@Composable
private fun CodeDialog(srv: ServerInfo, onDismiss: () -> Unit, onOk: (String) -> Unit) {
    var code by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("${srv.name} ile eşleş") },
        text = {
            Column {
                Text("Bilgisayar ekranındaki 6 haneli kodu gir.")
                Spacer(Modifier.size(10.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { v -> code = v.filter(Char::isDigit).take(6) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    placeholder = { Text("000000") },
                )
            }
        },
        confirmButton = { TextButton({ onOk(code) }, enabled = code.length == 6) { Text("Eşleştir") } },
        dismissButton = { TextButton(onDismiss) { Text("Vazgeç") } },
    )
}

private data class Perm(val title: String, val desc: String, val ok: Boolean, val action: (Context) -> Unit)

@Composable
private fun PermissionsCard() {
    val ctx = LocalContext.current
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
            add(Perm("MIUI: Otomatik başlatma", "Telefon açılınca ve arka planda çalışabilmek için “Otomatik başlat”ı aç.", false) { openMiuiAutostart(it) })
        }
    }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Kurulum", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            perms.forEach { p ->
                Row(
                    Modifier.fillMaxWidth().clickable(enabled = !p.ok) { p.action(ctx) }.padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        if (p.ok) Icons.Outlined.CheckCircle else Icons.Outlined.WarningAmber, null,
                        tint = if (p.ok) Color(0xFF16A34A) else Color(0xFFD97706),
                    )
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(p.title, fontWeight = FontWeight.Medium)
                        Text(p.desc, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    if (!p.ok) Text("Aç", color = MaterialTheme.colorScheme.primary)
                }
            }
        }
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
