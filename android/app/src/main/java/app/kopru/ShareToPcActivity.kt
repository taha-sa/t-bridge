package app.kopru

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.ui.res.stringResource
import app.kopru.ui.InviteApp
import app.kopru.ui.InviteDialog
import app.kopru.ui.TBridgeTheme
import app.kopru.ui.shareInviteLink
import org.json.JSONObject

/**
 * "Bilgisayarda indir": paylaşım menüsünden (ACTION_SEND text/plain) veya bağlantı seçicisinden (ACTION_VIEW http/https) gelen
 * adresi, kısa bir onaydan sonra PC'deki T-Downloader'a yollar. Görünmez etkinlik; toast ile sonucu söyler. Tarayıcıyı ele geçirmez: yalnızca seçici listesinde görünür.
 * PC'de T-Downloader yoksa onay yerine davet gösterilir (× yalnızca bu daveti kapatır).
 */
class ShareToPcActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: Context) = super.attachBaseContext(Lang.wrap(newBase))

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val url = extractUrl(intent)
        if (url == null) {
            toast(getString(R.string.share_pc_no_link))
            finish()
            return
        }
        if (Core.node == null || !Suite.supported()) {
            toast(getString(R.string.share_pc_offline))
            finish()
            return
        }
        // Dışa açık hedef: rastgele bir uygulama sessizce indirme tetikleyemesin diye gönderimden önce kısa onay (adres + gönderen görünür).
        val from = senderLabel()
        val shown = if (url.length > 160) url.take(157) + "…" else url
        val missing = Suite.pcMissing("t-downloader")
        setContent {
            TBridgeTheme {
                if (missing) {
                    InviteDialog(
                        app = InviteApp.DOWNLOADER,
                        title = stringResource(R.string.inv_share_title),
                        body = stringResource(R.string.inv_share_body),
                        cta = stringResource(R.string.inv_pc_cta),
                        onCta = { shareInviteLink(this, InviteApp.DOWNLOADER); finish() },
                        onClose = { finish() },
                        secondary = stringResource(R.string.inv_cancel),
                        onSecondary = { finish() },
                    )
                } else {
                    AlertDialog(
                        onDismissRequest = { finish() },
                        title = { Text(stringResource(R.string.share_pc_title)) },
                        text = { Text(if (from != null) stringResource(R.string.share_pc_sender, from) + "\n\n" + shown else shown) },
                        confirmButton = {
                            TextButton(onClick = {
                                Suite.send("t-downloader", "download.add", JSONObject().put("url", url).toString()) { ok, _, err ->
                                    toast(if (ok) getString(R.string.share_pc_started) else (err ?: getString(R.string.eco_send_failed)))
                                }
                                finish()
                            }) { Text(stringResource(R.string.share_pc_go)) }
                        },
                        dismissButton = { TextButton(onClick = { finish() }) { Text(stringResource(R.string.inv_cancel)) } },
                    )
                }
            }
        }
    }

    /** Çağıran uygulamanın etiketi (Intent referrer varsa); bilinmiyorsa null. */
    private fun senderLabel(): String? = runCatching {
        val pkg = referrer?.takeIf { it.scheme == "android-app" }?.host ?: callingPackage ?: error("yok")
        packageManager.getApplicationLabel(packageManager.getApplicationInfo(pkg, 0)).toString()
    }.getOrNull()

    private fun toast(m: String) = Toast.makeText(applicationContext, m, Toast.LENGTH_LONG).show()

    companion object {
        private val URL_RE = Regex("""https?://[^\s<>"']+""")

        /** SEND: metindeki ilk http(s) adresi; VIEW: veri adresi. */
        fun extractUrl(i: Intent?): String? {
            if (i == null) return null
            if (i.action == Intent.ACTION_VIEW) return i.dataString?.takeIf { it.startsWith("http://") || it.startsWith("https://") }
            val text = i.getStringExtra(Intent.EXTRA_TEXT) ?: return null
            return URL_RE.find(text)?.value?.trimEnd('.', ',', ')', ']')
        }
    }
}
