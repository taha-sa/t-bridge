package app.kopru.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import app.kopru.R

/** Ekosistemdeki T-Suite uygulamaları: simge, resmi indirme sayfası ve kendi tema rengi. */
enum class InviteApp(val id: String, @DrawableRes val icon: Int, val url: String, val accent: Color) {
    DOWNLOADER("t-downloader", R.drawable.app_t_downloader, "https://github.com/taha-sa/t-downloader-releases/releases/latest", Color(0xFFF5C542)),
    RAMCASH("t-ramcash", R.drawable.app_t_ramcash, "https://github.com/taha-sa/t-ramcash-releases/releases/latest", Color(0xFFC8921F)),
    DESKTREE("t-desktree", R.drawable.app_t_desktree, "https://github.com/taha-sa/t-desktree-releases/releases/latest", Color(0xFF2DD4BF)),
    VIEWER("t-desk-viewer", R.drawable.app_t_desk_viewer, "https://github.com/taha-sa/tdesk-viewer-releases/releases/latest", Color(0xFF8B9CFF)),
}

/** Resmi indirme sayfasını tarayıcıda açar. */
fun openInviteUrl(ctx: Context, app: InviteApp) {
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(app.url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

/** PC uygulaması için bağlantıyı paylaş/kopyala (paylaşım menüsünden bilgisayara iletilebilir). */
fun shareInviteLink(ctx: Context, app: InviteApp) {
    val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, app.url)
    runCatching { ctx.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

/**
 * Eksik uygulama daveti (satır içi kart): özelliğin kartı yerinde kalır; × yalnızca bu örneği kapatır,
 * kalıcı gizleme yoktur.
 */
@Composable
fun InviteCard(app: InviteApp, title: String, body: String, cta: String, onCta: () -> Unit, onClose: () -> Unit) {
    val c = tb
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(app.accent.copy(alpha = 0.08f))
            .border(1.dp, app.accent.copy(alpha = 0.35f), RoundedCornerShape(12.dp)).padding(start = 12.dp, top = 12.dp, bottom = 12.dp, end = 4.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Image(painterResource(app.icon), null, Modifier.size(40.dp).clip(RoundedCornerShape(8.dp)))
        Column(Modifier.weight(1f).padding(start = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, color = c.text, style = MaterialTheme.typography.titleSmall)
            Text(body, color = c.muted, style = MaterialTheme.typography.bodySmall)
            Box(
                Modifier.padding(top = 6.dp).clip(RoundedCornerShape(8.dp)).background(app.accent).clickable(onClick = onCta)
                    .padding(horizontal = 14.dp, vertical = 8.dp),
            ) { Text(cta, color = Color(0xFF1A1204), style = MaterialTheme.typography.labelLarge) }
        }
        CloseX(onClose)
    }
}

@Composable
private fun CloseX(onClose: () -> Unit) {
    Box(Modifier.size(40.dp).clip(RoundedCornerShape(16.dp)).clickable(onClick = onClose), contentAlignment = Alignment.Center) {
        Icon(Icons.Filled.Close, stringResource(R.string.invite_close), tint = tb.muted, modifier = Modifier.size(18.dp))
    }
}

/**
 * Eksik uygulama daveti (diyalog): simge, başlık, tek cümle, "… edin" düğmesi ve ×.
 * [secondary] verilirse ikinci bir seçenek (ör. "Başka uygulamayla aç") gösterilir.
 */
@Composable
fun InviteDialog(
    app: InviteApp, title: String, body: String, cta: String, onCta: () -> Unit, onClose: () -> Unit,
    secondary: String? = null, onSecondary: (() -> Unit)? = null,
) {
    val c = tb
    Dialog(onDismissRequest = onClose) {
        Box(
            Modifier.clip(RoundedCornerShape(16.dp)).background(c.s1).border(1.dp, app.accent.copy(alpha = 0.4f), RoundedCornerShape(16.dp)),
        ) {
            Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Image(painterResource(app.icon), null, Modifier.size(64.dp).clip(RoundedCornerShape(16.dp)))
                Text(title, color = c.text, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
                Text(body, color = c.muted, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center)
                Box(
                    Modifier.padding(top = 6.dp).clip(RoundedCornerShape(12.dp)).background(app.accent).clickable(onClick = onCta)
                        .padding(horizontal = 22.dp, vertical = 11.dp),
                ) { Text(cta, color = Color(0xFF1A1204), style = MaterialTheme.typography.labelLarge) }
                if (secondary != null && onSecondary != null) {
                    Text(secondary, color = c.muted, style = MaterialTheme.typography.labelLarge, modifier = Modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = onSecondary).padding(10.dp))
                }
            }
            Box(Modifier.align(Alignment.TopEnd).padding(6.dp)) { CloseX(onClose) }
        }
    }
}
