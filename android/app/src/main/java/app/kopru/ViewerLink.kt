package app.kopru

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Binder
import android.os.Bundle
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.Message
import android.os.Messenger
import android.webkit.MimeTypeMap
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.json.JSONObject
import uniffi.kopru_core.PcCommand
import uniffi.kopru_core.SpecialKey
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale
import java.util.concurrent.Executors

data class ViewerState(val installed: Boolean = false, val version: String = "", val update: String? = null)

/**
 * T-Desk Viewer (com.tdesk.viewer) ile köprü: kurulu mu / sürümü / güncelleme var mı, "Telefonda aç" ve sunum kumandası kanalı.
 * Güvenlik: bağlanılan servis yalnızca Viewer paketinin UID'sinden gelen iletileri işler (imza anahtarları farklı olduğundan imza izni yok).
 */
object ViewerLink {
    const val PKG = "com.tdesk.viewer"
    /** Viewer'ın kendi güncelleyicisiyle aynı kaynak: herkese açık releases deposu, GitHub "latest" yayını. */
    private const val LATEST_API = "https://api.github.com/repos/taha-sa/tdesk-viewer-releases/releases/latest"
    private const val CHECK_TTL_MS = 6 * 60 * 60 * 1000L

    private val _state = MutableStateFlow(ViewerState())
    val state: StateFlow<ViewerState> = _state
    private val io = Executors.newSingleThreadExecutor()
    private var lastCheck = 0L

    /** Viewer'ın (src/services/fileService.ts) açabildiği uzantılar. */
    val SUPPORTED: Set<String> = buildSet {
        addAll(listOf("pdf", "docx", "pptx", "epub", "zip", "txt", "log", "text", "md", "markdown", "mdown", "mkd"))
        addAll(listOf("xlsx", "xlsm", "xlsb", "xls", "ods", "fods", "csv", "tsv"))
        addAll(listOf("png", "jpg", "jpeg", "webp", "svg", "gif", "bmp", "ico"))
        addAll(
            listOf(
                "js", "jsx", "mjs", "cjs", "ts", "tsx", "html", "htm", "xml", "css", "scss", "sass", "less", "json", "jsonc", "json5",
                "yaml", "yml", "toml", "py", "pyw", "dart", "java", "kt", "kts", "groovy", "gradle", "c", "h", "cpp", "cc", "cxx", "hpp", "hh",
                "cs", "go", "rs", "php", "rb", "swift", "vue", "svelte", "tex", "sql", "sh", "bash", "zsh", "bat", "cmd", "ps1", "lua", "r",
                "scala", "pl", "ini", "env", "conf", "cfg", "properties",
            ),
        )
    }

    /** "Bu dosya bitince aç" bekleyenler (PC `file.open` olayı): küçük harfli ad -> zaman. */
    private val pendingOpen = HashMap<String, Long>()
    private const val PENDING_TTL_MS = 15 * 60 * 1000L

    fun isInstalled(ctx: Context): Boolean = versionOf(ctx) != null

    private fun versionOf(ctx: Context): String? = try {
        ctx.packageManager.getPackageInfo(PKG, 0).versionName ?: ""
    } catch (_: PackageManager.NameNotFoundException) {
        null
    }

    fun supports(name: String): Boolean = name.substringAfterLast('.', "").lowercase(Locale.ROOT) in SUPPORTED

    /** Kurulum + sürüm durumunu yeniler; ağ denetimi en çok 6 saatte bir (zorlanırsa hemen). */
    fun refresh(ctx: Context, force: Boolean = false) {
        val app = ctx.applicationContext
        val v = versionOf(app)
        if (v == null) {
            _state.value = ViewerState()
            return
        }
        _state.update { it.copy(installed = true, version = v, update = if (it.version == v) it.update else null) }
        val now = System.currentTimeMillis()
        if (!force && now - lastCheck < CHECK_TTL_MS) return
        lastCheck = now
        io.execute {
            val latest = runCatching { fetchLatestTag() }.getOrNull() ?: return@execute
            val newer = if (compare(latest, v) > 0) latest else null
            _state.update { it.copy(update = newer) }
        }
    }

    private fun fetchLatestTag(): String? {
        val c = URL(LATEST_API).openConnection() as HttpURLConnection
        try {
            c.connectTimeout = 10_000
            c.readTimeout = 10_000
            c.setRequestProperty("Accept", "application/vnd.github+json")
            if (c.responseCode != 200) return null
            val o = JSONObject(c.inputStream.bufferedReader().use { it.readText() })
            if (o.optBoolean("draft") || o.optBoolean("prerelease")) return null
            return o.optString("tag_name").removePrefix("v").removePrefix("V").takeIf { it.isNotEmpty() }
        } finally {
            c.disconnect()
        }
    }

    /** "1.2.3" biçimli sürümleri karşılaştırır (viewer'ın compareVersions'ı ile aynı kural); çözülemezse 0. */
    fun compare(a: String, b: String): Int {
        fun parse(s: String): List<Int>? {
            val m = Regex("""^[vV]?(\d+)(?:\.(\d+))?(?:\.(\d+))?""").find(s.trim()) ?: return null
            return m.groupValues.drop(1).map { it.toIntOrNull() ?: 0 }
        }
        val pa = parse(a) ?: return 0
        val pb = parse(b) ?: return 0
        for (i in 0 until 3) {
            if (pa[i] != pb[i]) return pa[i].compareTo(pb[i])
        }
        return 0
    }

    fun launchViewer(ctx: Context): Boolean {
        val i = ctx.packageManager.getLaunchIntentForPackage(PKG) ?: return false
        ctx.startActivity(i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        return true
    }

    /** Viewer'ın güncelleme ekranını derin bağlantıyla açar (tdeskviewer://update). */
    fun openUpdateScreen(ctx: Context): Boolean = runCatching {
        ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("tdeskviewer://update")).setPackage(PKG).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        true
    }.getOrDefault(false)

    // ---------------------------------------------------------------- "Telefonda aç"

    fun expectOpen(name: String) = synchronized(pendingOpen) {
        val now = System.currentTimeMillis()
        pendingOpen.entries.removeAll { now - it.value > PENDING_TTL_MS }
        pendingOpen[name.substringAfterLast('/').lowercase(Locale.ROOT)] = now
    }

    fun consumeOpen(name: String): Boolean = synchronized(pendingOpen) {
        val t = pendingOpen.remove(name.substringAfterLast('/').lowercase(Locale.ROOT)) ?: return false
        System.currentTimeMillis() - t <= PENDING_TTL_MS
    }

    fun mimeOf(ctx: Context, uri: Uri, name: String): String {
        val ext = name.substringAfterLast('.', "").lowercase(Locale.ROOT)
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext)
            ?: runCatching { ctx.contentResolver.getType(uri) }.getOrNull()
            ?: "*/*"
    }

    /** Viewer kuruluysa ve uzantı destekleniyorsa doğrudan Viewer'da, değilse sistem seçicisinde aç. */
    fun open(ctx: Context, uri: Uri, name: String): Boolean {
        val mime = mimeOf(ctx, uri, name)
        val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        view.clipData = android.content.ClipData.newRawUri(name, uri)
        return try {
            if (isInstalled(ctx) && supports(name)) {
                ctx.startActivity(view.setPackage(PKG))
            } else {
                ctx.startActivity(Intent.createChooser(view, "Aç").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            }
            true
        } catch (_: Exception) {
            false
        }
    }

    /** Bildirime dokununca aynı hedef (uygulama arkadayken doğrudan açma Android tarafından engellenebilir). */
    fun openIntent(ctx: Context, uri: Uri, name: String): Intent {
        val mime = mimeOf(ctx, uri, name)
        val i = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        if (isInstalled(ctx) && supports(name)) i.setPackage(PKG)
        return i
    }
}

/**
 * Viewer -> Bridge kanalı (Messenger). Yalnızca com.tdesk.viewer UID'sinden gelen iletiler işlenir; diğerlerine "reddedildi" döner.
 *  what=1 SEND   : uri (grantUriPermission ile verilmiş), name, present(bool)  -> dosyayı PC'ye gönderir (present: PC'de tam ekran sunum)
 *  what=2 KEY    : key = "next" | "prev" | "end"                                -> PC'ye ok tuşu / Esc
 *  what=3 PING   : kurulum/bağlantı durumu
 * Yanıt: replyTo'ya what=100, data {ok:Boolean, msg:String, connected:Boolean}.
 */
class ViewerBridgeService : Service() {
    private lateinit var messenger: Messenger

    private val handler = object : Handler(Looper.getMainLooper()) {
        override fun handleMessage(msg: Message) {
            val reply = msg.replyTo
            if (!callerIsViewer(msg.sendingUid)) {
                respond(reply, false, "Reddedildi")
                return
            }
            when (msg.what) {
                WHAT_SEND -> handleSend(msg.data, reply)
                WHAT_KEY -> {
                    val k = when (msg.data.getString("key")) {
                        "next" -> SpecialKey.RIGHT
                        "prev" -> SpecialKey.LEFT
                        "end" -> SpecialKey.ESCAPE
                        else -> null
                    }
                    val ok = k != null && Core.isConnected() && Core.node?.sendPc(PcCommand.Key(k)) == true
                    respond(reply, ok, if (ok) "" else "Bilgisayar bağlı değil")
                }
                WHAT_PING -> respond(reply, true, "")
                else -> respond(reply, false, "Bilinmeyen komut")
            }
        }
    }

    private fun callerIsViewer(uid: Int): Boolean {
        if (uid <= 0) return false
        val pkgs = packageManager.getPackagesForUid(uid) ?: return false
        return ViewerLink.PKG in pkgs
    }

    private fun handleSend(d: Bundle, reply: Messenger?) {
        val uri = d.getString("uri")?.let(Uri::parse)
        if (uri == null) { respond(reply, false, "Dosya yok"); return }
        val file = OutgoingFiles.describe(this, uri, persist = false)
        if (file == null) { respond(reply, false, "Dosya okunamadı"); return }
        KopruService.startHolding(this, listOf(uri))
        val present = d.getBoolean("present", false)
        if (present) {
            if (!Core.isConnected() || !Suite.supported()) { respond(reply, false, "Bilgisayar bağlı değil veya sürüm eski"); return }
            // PC önce adı öğrenir; dosya bitince tam ekran açar.
            Suite.send("t-bridge", "file.present", JSONObject().put("name", file.name.substringAfterLast('/')).toString())
        }
        val n = Core.sendFiles(listOf(file))
        when {
            n == 0 -> respond(reply, false, "Gönderilemedi (eşleşmiş bilgisayar yok?)")
            Core.isConnected() -> respond(reply, true, if (present) "Sunum bilgisayara gönderiliyor" else "Dosya gönderiliyor")
            else -> respond(reply, true, "Sıraya alındı; bilgisayar bağlanınca gidecek")
        }
    }

    private fun respond(to: Messenger?, ok: Boolean, text: String) {
        to ?: return
        val m = Message.obtain(null, WHAT_RESULT)
        m.data = Bundle().apply {
            putBoolean("ok", ok)
            putString("msg", text)
            putBoolean("connected", Core.isConnected())
        }
        runCatching { to.send(m) }
    }

    override fun onCreate() {
        super.onCreate()
        Core.start()
        messenger = Messenger(handler)
    }

    override fun onBind(intent: Intent?): IBinder = messenger.binder

    companion object {
        const val WHAT_SEND = 1
        const val WHAT_KEY = 2
        const val WHAT_PING = 3
        const val WHAT_RESULT = 100
    }
}
