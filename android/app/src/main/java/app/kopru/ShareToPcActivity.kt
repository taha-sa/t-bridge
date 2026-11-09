package app.kopru

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import org.json.JSONObject

/**
 * "Bilgisayarda indir": paylaşım menüsünden (ACTION_SEND text/plain) veya bağlantı seçicisinden (ACTION_VIEW http/https) gelen
 * adresi, kısa bir onaydan sonra PC'deki T-Downloader'a yollar. Görünmez etkinlik; toast ile sonucu söyler. Tarayıcıyı ele geçirmez: yalnızca seçici listesinde görünür.
 */
class ShareToPcActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val url = extractUrl(intent)
        if (url == null) {
            toast("Paylaşılan içerikte bağlantı bulunamadı")
            finish()
            return
        }
        if (Core.node == null || !Suite.supported()) {
            toast("Bilgisayara bağlı değil")
            finish()
            return
        }
        // Dışa açık hedef: rastgele bir uygulama sessizce indirme tetikleyemesin diye gönderimden önce kısa onay (adres + gönderen görünür).
        val from = senderLabel()
        val shown = if (url.length > 160) url.take(157) + "…" else url
        AlertDialog.Builder(this)
            .setTitle("Bilgisayarda indirilsin mi?")
            .setMessage(if (from != null) "Gönderen: ${from}" + "\u000A\u000A" + shown else shown)
            .setPositiveButton("İndir") { _, _ ->
                Suite.send("t-downloader", "download.add", JSONObject().put("url", url).toString()) { ok, _, err ->
                    toast(if (ok) "Bilgisayarda indirme başladı" else (err ?: "Gönderilemedi"))
                }
                finish()
            }
            .setNegativeButton("Vazgeç") { _, _ -> finish() }
            .setOnCancelListener { finish() }
            .show()
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
