@file:Suppress("LocalContextGetResourceValueCall") // metinler tıklama/sonuç geri çağrılarında çözülür; dil değişince etkinlik yeniden oluşturulur

package app.kopru.ui

import androidx.compose.ui.res.stringResource
import app.kopru.R
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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.InsertDriveFile
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.Archive
import androidx.compose.material.icons.outlined.ArrowDownward
import androidx.compose.material.icons.outlined.ArrowUpward
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.MusicNote
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Upload
import androidx.compose.material.icons.outlined.Videocam
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
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

private fun kindIcon(name: String): ImageVector = when (name.substringAfterLast('.', "").lowercase()) {
    "jpg", "jpeg", "png", "gif", "webp", "heic", "bmp", "svg" -> Icons.Outlined.Image
    "mp4", "mov", "mkv", "avi", "webm", "3gp" -> Icons.Outlined.Videocam
    "mp3", "m4a", "wav", "flac", "ogg", "aac", "opus" -> Icons.Outlined.MusicNote
    "zip", "rar", "7z", "tar", "gz", "apk" -> Icons.Outlined.Archive
    else -> Icons.AutoMirrored.Outlined.InsertDriveFile
}

@Composable
fun FilesScreen(ui: UiState) {
    val ctx = LocalContext.current
    val c = tb
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        val files = uris.mapNotNull { OutgoingFiles.describe(ctx, it, persist = true) }
        val n = Core.sendFiles(files)
        val msg = if (n == 0) ctx.getString(R.string.files_toast_failed) else if (Core.isConnected()) ctx.resources.getQuantityString(R.plurals.files_toast_sending, n, n) else ctx.resources.getQuantityString(R.plurals.files_toast_queued, n, n)
        Toast.makeText(ctx, msg, Toast.LENGTH_SHORT).show()
    }
    val all = ui.transfers.values.toList()
    val groupRunning = stringResource(R.string.files_group_running)
    val groupWaiting = stringResource(R.string.files_group_waiting)
    val running = all.filter { it.state == TransferState.ACTIVE }
    val waiting = all.filter { it.state == TransferState.QUEUED || it.state == TransferState.PAUSED }
    val finished = all.filter { it.state in setOf(TransferState.DONE, TransferState.FAILED, TransferState.CANCELLED) }.sortedByDescending { it.id }
    val on = ui.conn == ConnState.CONNECTED

    LazyColumn(contentPadding = PaddingValues(horizontal = 18.dp, vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item {
            Text(stringResource(R.string.files_title), color = c.text, style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(top = 4.dp, bottom = 6.dp))
        }
        item {
            TbCard(gold = on) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconTile(Icons.Outlined.Upload, size = 46.dp)
                    Spacer(Modifier.width(14.dp))
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.files_send_title), color = c.text, style = MaterialTheme.typography.titleMedium)
                        Text(
                            when {
                                ui.peers.isEmpty() -> stringResource(R.string.files_hint_pair_first)
                                on -> stringResource(R.string.files_hint_ready, ui.peer?.name ?: stringResource(R.string.files_computer_default))
                                else -> stringResource(R.string.files_hint_offline)
                            },
                            color = c.muted,
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
                Spacer(Modifier.height(14.dp))
                GoldButton(stringResource(R.string.files_send_button), { picker.launch(arrayOf("*/*")) }, Modifier.fillMaxWidth(), icon = Icons.Outlined.Upload, enabled = ui.peers.isNotEmpty())
                Spacer(Modifier.height(10.dp))
                Text(
                    stringResource(R.string.files_incoming_hint),
                    color = c.faint,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
        if (all.isEmpty()) {
            item {
                Column(Modifier.fillMaxWidth().padding(vertical = 40.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    IconTile(Icons.AutoMirrored.Outlined.InsertDriveFile, tint = c.muted, bg = c.s3, border = c.line, size = 56.dp)
                    Spacer(Modifier.height(12.dp))
                    Text(stringResource(R.string.files_empty), color = c.text2, style = MaterialTheme.typography.titleSmall)
                }
            }
        }
        group(groupRunning, running, on)
        group(groupWaiting, waiting, on)
        if (finished.isNotEmpty()) {
            item(key = "h-done") {
                Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    SectionTitle(stringResource(R.string.files_group_done), Modifier.weight(1f))
                    TextButton({ Core.node?.clearFinished() }) { Text(stringResource(R.string.files_clear), color = c.goldHi) }
                }
            }
            items(finished, key = { Core.tkey(it) }) { t -> TransferRow(t, on) }
        }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.group(title: String, list: List<TransferInfo>, on: Boolean) {
    if (list.isEmpty()) return
    item(key = "h-$title") { SectionTitle(title, Modifier.padding(top = 8.dp)) }
    items(list.sortedByDescending { it.id }, key = { Core.tkey(it) }) { t -> TransferRow(t, on) }
}

@Composable
private fun TransferRow(t: TransferInfo, connected: Boolean) {
    val ctx = LocalContext.current
    val c = tb
    val size = t.size.toLong()
    val done = t.done.toLong()
    val p = if (size == 0L) (if (t.state == TransferState.DONE) 1f else 0f) else (done.toFloat() / size).coerceIn(0f, 1f)
    val incoming = t.direction == Direction.INCOMING
    val label = when (t.state) {
        TransferState.QUEUED -> if (connected) stringResource(R.string.files_state_queued) else stringResource(R.string.files_state_waiting_conn)
        TransferState.ACTIVE -> "${fmtBytes(t.speed.toLong())}/s"
        TransferState.PAUSED -> stringResource(R.string.files_state_paused)
        TransferState.DONE -> stringResource(R.string.files_state_done)
        TransferState.FAILED -> t.detail?.let { stringResource(R.string.files_state_failed_detail, it) } ?: stringResource(R.string.files_state_failed)
        TransferState.CANCELLED -> stringResource(R.string.files_state_cancelled)
    }
    val labelColor = when (t.state) {
        TransferState.ACTIVE -> c.goldHi
        TransferState.DONE -> c.ok
        TransferState.FAILED -> c.bad
        else -> c.muted
    }
    val name = t.name.substringAfterLast('/')
    TbCard(gold = t.state == TransferState.ACTIVE, padding = PaddingValues(start = 14.dp, top = 12.dp, end = 6.dp, bottom = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconTile(kindIcon(name), tint = c.text2, bg = c.s3, border = c.line, size = 42.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(name, color = c.text, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Spacer(Modifier.height(2.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(if (incoming) Icons.Outlined.ArrowDownward else Icons.Outlined.ArrowUpward, null, tint = c.muted, modifier = Modifier.size(13.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("${fmtBytes(done)} / ${fmtBytes(size)}", color = c.muted, style = MaterialTheme.typography.bodySmall)
                }
            }
            val n = Core.node
            when (t.state) {
                TransferState.ACTIVE, TransferState.QUEUED ->
                    IconButton({ runCatching { n?.pauseTransfer(t.id, t.direction) } }) { Icon(Icons.Outlined.Pause, stringResource(R.string.files_cd_pause), tint = c.text2) }
                TransferState.PAUSED ->
                    IconButton({ runCatching { n?.resumeTransfer(t.id, t.direction) } }) { Icon(Icons.Outlined.PlayArrow, stringResource(R.string.files_cd_resume), tint = c.goldHi) }
                TransferState.DONE -> if (incoming && t.detail != null) {
                    IconButton({
                        IncomingFiles.findByPath(t.detail!!)?.let { uri ->
                            val i = Intent(Intent.ACTION_VIEW).setDataAndType(uri, ctx.contentResolver.getType(uri) ?: "*/*")
                                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            runCatching { ctx.startActivity(i) }.onFailure { Toast.makeText(ctx, ctx.getString(R.string.files_toast_no_app), Toast.LENGTH_SHORT).show() }
                        }
                    }) { Icon(Icons.AutoMirrored.Outlined.OpenInNew, stringResource(R.string.files_cd_open), tint = c.goldHi) }
                }
                else -> {}
            }
            if (t.state in setOf(TransferState.ACTIVE, TransferState.QUEUED, TransferState.PAUSED)) {
                IconButton({ n?.cancelTransfer(t.id, t.direction) }) { Icon(Icons.Outlined.Close, stringResource(R.string.files_cd_cancel), tint = c.muted) }
            }
        }
        Column(Modifier.padding(end = 8.dp, top = 8.dp)) {
            if (t.state != TransferState.DONE && t.state != TransferState.CANCELLED) {
                GoldProgress(
                    p,
                    active = t.state == TransferState.ACTIVE,
                    tone = if (t.state == TransferState.ACTIVE) "gold" else if (t.state == TransferState.FAILED) "bad" else "muted",
                )
                Spacer(Modifier.height(6.dp))
            }
            Text(label, color = labelColor, style = MaterialTheme.typography.labelMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
    }
}
