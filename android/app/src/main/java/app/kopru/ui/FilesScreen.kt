package app.kopru.ui

import android.content.Intent
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ArrowDownward
import androidx.compose.material.icons.outlined.ArrowUpward
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Upload
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.kopru.Core
import app.kopru.IncomingFiles
import app.kopru.OutgoingFiles
import app.kopru.UiState
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.Direction
import uniffi.kopru_core.TransferInfo
import uniffi.kopru_core.TransferState

@Composable
fun FilesScreen(ui: UiState) {
    val ctx = LocalContext.current
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        val files = uris.mapNotNull { OutgoingFiles.describe(ctx, it, persist = true) }
        val n = Core.sendFiles(files)
        val msg = if (n == 0) "Gönderilemedi" else if (Core.isConnected()) "$n dosya gönderiliyor" else "$n dosya sıraya alındı"
        Toast.makeText(ctx, msg, Toast.LENGTH_SHORT).show()
    }
    val order = mapOf(TransferState.ACTIVE to 0, TransferState.QUEUED to 1, TransferState.PAUSED to 2, TransferState.FAILED to 3, TransferState.DONE to 4, TransferState.CANCELLED to 5)
    val list = ui.transfers.values.sortedWith(compareBy<TransferInfo> { order[it.state] }.thenByDescending { it.id })
    val anyFinished = list.any { it.state in setOf(TransferState.DONE, TransferState.FAILED, TransferState.CANCELLED) }

    LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Button({ picker.launch(arrayOf("*/*")) }, Modifier.fillMaxWidth(), enabled = ui.peers.isNotEmpty()) {
                Icon(Icons.Outlined.Upload, null)
                Spacer(Modifier.width(8.dp))
                Text("Bilgisayara dosya gönder")
            }
            Text(
                "Gelen dosyalar: İndirilenler/Köprü. Herhangi bir uygulamadan “Paylaş → Köprü” ile de gönderebilirsin.",
                Modifier.padding(top = 8.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (anyFinished) {
            item {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    TextButton({ Core.node?.clearFinished() }) { Text("Bitenleri temizle") }
                }
            }
        }
        if (list.isEmpty()) {
            item { Text("Henüz aktarım yok.", Modifier.padding(24.dp), color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
        items(list, key = { Core.tkey(it) }) { t -> TransferRow(t, ui.conn == ConnState.CONNECTED) }
    }
}

@Composable
private fun TransferRow(t: TransferInfo, connected: Boolean) {
    val ctx = LocalContext.current
    val size = t.size.toLong()
    val done = t.done.toLong()
    val p = if (size == 0L) (if (t.state == TransferState.DONE) 1f else 0f) else (done.toFloat() / size).coerceIn(0f, 1f)
    val label = when (t.state) {
        TransferState.QUEUED -> if (connected) "Sırada" else "Bağlantı bekleniyor"
        TransferState.ACTIVE -> "${fmtBytes(t.speed.toLong())}/s"
        TransferState.PAUSED -> "Duraklatıldı"
        TransferState.DONE -> "Tamamlandı"
        TransferState.FAILED -> "Başarısız" + (t.detail?.let { ": $it" } ?: "")
        TransferState.CANCELLED -> "İptal edildi"
    }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(if (t.direction == Direction.INCOMING) Icons.Outlined.ArrowDownward else Icons.Outlined.ArrowUpward, null)
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(t.name.substringAfterLast('/'), fontWeight = FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text("${fmtBytes(done)} / ${fmtBytes(size)} · $label", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2)
                }
                val n = Core.node
                when (t.state) {
                    TransferState.ACTIVE, TransferState.QUEUED ->
                        IconButton({ runCatching { n?.pauseTransfer(t.id, t.direction) } }) { Icon(Icons.Outlined.Pause, "Duraklat") }
                    TransferState.PAUSED ->
                        IconButton({ runCatching { n?.resumeTransfer(t.id, t.direction) } }) { Icon(Icons.Outlined.PlayArrow, "Devam") }
                    TransferState.DONE -> if (t.direction == Direction.INCOMING && t.detail != null) {
                        IconButton({
                            IncomingFiles.findByPath(t.detail!!)?.let { uri ->
                                val i = Intent(Intent.ACTION_VIEW).setDataAndType(uri, ctx.contentResolver.getType(uri) ?: "*/*")
                                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                runCatching { ctx.startActivity(i) }.onFailure { Toast.makeText(ctx, "Açacak uygulama yok", Toast.LENGTH_SHORT).show() }
                            }
                        }) { Icon(Icons.AutoMirrored.Outlined.OpenInNew, "Aç") }
                    }
                    else -> {}
                }
                if (t.state in setOf(TransferState.ACTIVE, TransferState.QUEUED, TransferState.PAUSED)) {
                    IconButton({ n?.cancelTransfer(t.id, t.direction) }) { Icon(Icons.Outlined.Close, "İptal") }
                }
            }
            if (t.state != TransferState.DONE && t.state != TransferState.CANCELLED) {
                LinearProgressIndicator(progress = { p }, modifier = Modifier.fillMaxWidth().padding(top = 8.dp))
            }
        }
    }
}
