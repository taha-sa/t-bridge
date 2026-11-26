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
        io.execute {
            val latest = runCatching { fetchLatestTag() }.getOrNull() ?: return@execute
            lastCheck = System.currentTimeMillis()
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

    // ---------------------------------------------------------------- imza sertifikası sabitleme

    /** "AA:BB cc" -> "aabbcc" (küçük harf, yalnız hex). */
    fun normDigest(s: String): String = s.filter { it.isLetterOrDigit() }.lowercase(Locale.ROOT)

    /** Paketin imzalayan sertifikalarının SHA-256 özetleri (küçük harf hex); okunamazsa boş. */
    fun signerDigests(ctx: Context, pkg: String): Set<String> = runCatching {
        val info = ctx.packageManager.getPackageInfo(pkg, PackageManager.GET_SIGNING_CERTIFICATES)
        val signers = info.signingInfo?.apkContentsSigners ?: return emptySet()
        val md = java.security.MessageDigest.getInstance("SHA-256")
        signers.map { sig -> md.digest(sig.toByteArray()).joinToString("") { "%02x".format(it) } }.toSet()
    }.getOrDefault(emptySet())

    /**
     * Çağıranın imzası yapılandırılan parmak izlerinden biriyle (virgülle ayrılmış) eşleşmeli ve tüm imzacılar izinli olmalı.
     * Parmak izi yapılandırılmamışsa: debug yapıda esnek (paket adı yeter), release yapıda reddedilir. Saf mantık.
     */
    fun certAllowed(actual: Set<String>, pinnedCsv: String, lenient: Boolean): Boolean {
        val pinned = pinnedCsv.split(',').map(::normDigest).filter { it.isNotEmpty() }.toSet()
        if (pinned.isEmpty()) return lenient
        return actual.isNotEmpty() && actual.all { it in pinned }
    }

    // ---------------------------------------------------------------- sunum durumu (what=101)

    private class PresentWait(val reply: Messenger, val at: Long)
    private val presentWait = HashMap<String, PresentWait>()

    private fun key(name: String) = name.substringAfterLast('/').lowercase(Locale.ROOT)

    /** Sunum gönderimi başladı: Viewer'a `transferring` yollar ve PC'nin açma sonucunu bekler. */
    fun trackPresent(name: String, reply: Messenger?) {
        reply ?: return
        synchronized(presentWait) {
            val now = System.currentTimeMillis()
            presentWait.entries.removeAll { now - it.value.at > PENDING_TTL_MS }
            presentWait[key(name)] = PresentWait(reply, now)
        }
        sendPresentState(reply, "transferring", s(R.string.viewer_present_sending))
    }

    /** Terminal durum (opened/failed): bekleyeni kaldırır ve Viewer'a bildirir. Bekleyen yoksa yok sayar. */
    fun finishPresent(name: String, ok: Boolean, failMsg: String? = null) {
        val w = synchronized(presentWait) { presentWait.remove(key(name)) } ?: return
        if (ok) sendPresentState(w.reply, "opened", s(R.string.viewer_present_opened))
        else sendPresentState(w.reply, "failed", failMsg ?: s(R.string.viewer_present_failed))
    }

    /** PC `file.presented` olayı. */
    fun onPresented(name: String, ok: Boolean) = finishPresent(name, ok)

    /** Giden aktarım başarısız/iptal olduysa sunum da başarısızdır. */
    fun onOutgoingTransfer(name: String, failed: Boolean) {
        if (failed) finishPresent(name, false, s(R.string.viewer_send_failed))
    }

    /** PC bağlantısı koptuğunda bekleyen sunum isteklerini başarısız olarak sonlandır. */
    fun onDisconnected() {
        val pending = synchronized(presentWait) {
            val copy = presentWait.values.toList()
            presentWait.clear()
            copy
        }
        val msg = s(R.string.viewer_pc_off)
        pending.forEach { sendPresentState(it.reply, "failed", msg) }
    }

    private fun sendPresentState(to: Messenger, state: String, text: String) {
        val m = Message.obtain(null, ViewerBridgeService.WHAT_PRESENT_STATE)
        m.data = Bundle().apply {
            putString("state", state)
            putString("msg", text)
            putBoolean("connected", Core.isConnected())
        }
        runCatching { to.send(m) }
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
            } else if (supports(name)) {
                ctx.startActivity(ViewerInviteActivity.intent(ctx, uri, name, mime))
            } else {
                ctx.startActivity(Intent.createChooser(view, ctx.getString(R.string.viewer_chooser)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
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
        else if (supports(name)) return ViewerInviteActivity.intent(ctx, uri, name, mime)
        return i
    }
}

private fun s(@androidx.annotation.StringRes id: Int): String = Suite.appCtx().getString(id)

/**
 * Viewer -> Bridge kanalı (Messenger). Yalnızca com.tdesk.viewer UID'sinden gelen iletiler işlenir; diğerlerine "reddedildi" döner.
 *  what=1 SEND   : uri (grantUriPermission ile verilmiş), name, present(bool)  -> dosyayı PC'ye gönderir (present: PC'de tam ekran sunum)
 *  what=2 KEY    : key = "next" | "prev" | "end"                                -> PC'ye ok tuşu / Esc
 *  what=3 PING   : kurulum/bağlantı durumu
 * present=true ise ek olarak what=101 (state transferring -> opened|failed, msg, connected).
 * Yanıt: replyTo'ya what=100, data {ok:Boolean, msg:String, connected:Boolean}.
 */
class ViewerBridgeService : Service() {
    private lateinit var messenger: Messenger

    private val handler = object : Handler(Looper.getMainLooper()) {
        override fun handleMessage(msg: Message) {
            val reply = msg.replyTo
            if (!callerIsViewer(msg.sendingUid)) {
                respond(reply, false, s(R.string.viewer_denied))
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
                    respond(reply, ok, if (ok) "" else s(R.string.viewer_pc_off))
                }
                WHAT_PING -> respond(reply, true, "")
                else -> respond(reply, false, s(R.string.viewer_unknown_cmd))
            }
        }
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var cachedUid = -1
    private var cachedUpdated = -1L
    private var cachedAllowed = false

    private fun callerIsViewer(uid: Int): Boolean {
        if (uid <= 0) return false
        val pkgs = packageManager.getPackagesForUid(uid) ?: return false
        if (ViewerLink.PKG !in pkgs) return false
        val updated = runCatching { packageManager.getPackageInfo(ViewerLink.PKG, 0).lastUpdateTime }.getOrDefault(-1L)
        if (uid == cachedUid && updated >= 0L && updated == cachedUpdated) return cachedAllowed
        val allowed = ViewerLink.certAllowed(ViewerLink.signerDigests(this, ViewerLink.PKG), BuildConfig.VIEWER_CERT_SHA256, BuildConfig.DEBUG)
        cachedUid = uid
        cachedUpdated = updated
        cachedAllowed = allowed
        return allowed
    }

    private fun handleSend(d: Bundle, reply: Messenger?) {
        val uri = d.getString("uri")?.let(Uri::parse)
        if (uri == null) { respond(reply, false, s(R.string.viewer_no_file)); return }
        val file = OutgoingFiles.describe(this, uri, persist = false)
        if (file == null) { respond(reply, false, s(R.string.viewer_unreadable)); return }
        val present = d.getBoolean("present", false)
        if (present) {
            if (!Core.isConnected() || !Suite.supported()) { respond(reply, false, s(R.string.viewer_pc_off_or_old)); return }
            val pname = file.name.substringAfterLast('/')
            ViewerLink.trackPresent(pname, reply)
            startRegisteredPresentation(
                register = { done ->
                    Suite.send("t-bridge", "file.present", JSONObject().put("name", pname).toString()) { ok, _, err ->
                        mainHandler.post { done(ok, Suite.friendlyError(err)) }
                    }
                },
                send = {
                    if (!Core.isConnected()) false else {
                        KopruService.startHolding(this, listOf(uri))
                        val sent = Core.sendFiles(listOf(file)) > 0
                        if (sent) respond(reply, true, s(R.string.viewer_present_sending))
                        sent
                    }
                },
                failed = { err ->
                    val message = err ?: s(R.string.viewer_send_fail_nopc)
                    ViewerLink.finishPresent(pname, false, message)
                    respond(reply, false, message)
                },
            )
            return
        }
        KopruService.startHolding(this, listOf(uri))
        val n = Core.sendFiles(listOf(file))
        when {
            n == 0 -> respond(reply, false, s(R.string.viewer_send_fail_nopc))
            Core.isConnected() -> respond(reply, true, s(R.string.viewer_sending_file))
            else -> respond(reply, true, s(R.string.viewer_queued))
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
        /** Sunum yaşam döngüsü: Bundle state = transferring | opened | failed, msg = Türkçe metin, connected. */
        const val WHAT_PRESENT_STATE = 101
    }
}
