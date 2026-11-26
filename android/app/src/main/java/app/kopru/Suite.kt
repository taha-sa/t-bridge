package app.kopru

import android.annotation.SuppressLint
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.json.JSONObject
import uniffi.kopru_core.SuiteApp
import java.util.concurrent.atomic.AtomicLong

data class DlItem(val id: String, val name: String, val pct: Double, val speed: Double, val bytes: Long, val total: Long, val paused: Boolean = false) {
    /** Kalan süre (sn); hız ya da toplam bilinmiyorsa -1. */
    val etaSec: Long get() = if (speed > 1 && total > bytes) ((total - bytes) / speed).toLong() else -1
}

data class DlDone(val id: String, val name: String, val path: String, val size: Long)
data class RamInfo(val usedPct: Int, val usedMb: Long, val totalMb: Long)
data class DiskRow(val drive: String, val freeBytes: Long, val totalBytes: Long) {
    val usedPct: Float get() = if (totalBytes > 0) (1f - freeBytes.toFloat() / totalBytes).coerceIn(0f, 1f) else 0f
}

data class SuiteUi(
    val apps: List<SuiteApp> = emptyList(),
    val downloads: Map<String, DlItem> = emptyMap(),
    val completed: List<DlDone> = emptyList(),
    val ram: RamInfo? = null,
    val disks: List<DiskRow> = emptyList(),
    /** Bilinmeyen (gelecekteki) uygulamaların son olayları: uygulama kimliği -> satırlar. */
    val events: Map<String, List<String>> = emptyMap(),
    val message: String? = null,
    val notify: Boolean = true,
    val updating: Boolean = false,
)

/**
 * T-Suite ekosistemi (telefon): PC'deki T-Bridge hub'ı genel SuiteApps/SuiteEvent/SuiteCmd/SuiteReply mesajlarıyla uygulamaları aktarır.
 * Uygulamaya özgü protokol yok: bilinen olaylar özel kartlara, bilinmeyenler genel karta düşer.
 */
object Suite {
    private const val CH = "kopru_eco"
    private val KNOWN = setOf("t-ramcash", "t-downloader", "t-desktree", "t-bridge")
    private val _ui = MutableStateFlow(SuiteUi())
    val ui: StateFlow<SuiteUi> = _ui

    private lateinit var ctx: Context
    private val reqSeq = AtomicLong(1)
    private data class Reply(val ok: Boolean, val json: JSONObject?, val error: String?)
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private val FREE_RE = Regex("""free=(\d+)""")
    private val pending = PendingRequests<Reply>(
        schedule = { task, delay -> handler.postDelayed(task, delay) },
        cancel = { task -> handler.removeCallbacks(task) },
    )
    private var ramHighSince = 0L
    private var ramLastNotified = 0L

    /** Bağlantı koptu: bayat indirme ilerlemesini temizle (PC bağlanınca SuiteEvent ile yeniden dolar). */
    fun onDisconnected() {
        _ui.update { it.copy(downloads = emptyMap(), completed = emptyList()) }
        pending.failAll(Reply(false, null, appCtx().getString(R.string.suite_err_offline)))
        ViewerLink.onDisconnected()
    }

    fun init(c: Context) {
        ctx = c.applicationContext
        val p = ctx.getSharedPreferences("eco", Context.MODE_PRIVATE)
        _ui.update { it.copy(notify = p.getBoolean("notify", true)) }
        ctx.getSystemService(NotificationManager::class.java)
            .createNotificationChannel(NotificationChannel(CH, ctx.getString(R.string.nav_eco), NotificationManager.IMPORTANCE_DEFAULT))
    }

    fun setNotify(on: Boolean) {
        ctx.getSharedPreferences("eco", Context.MODE_PRIVATE).edit().putBoolean("notify", on).apply()
        _ui.update { it.copy(notify = on) }
    }

    fun supported() = Core.node?.peerSupportsSuite() == true

    fun appCtx(): Context = ctx

    /** PC uygulama listesi geldi ve [id] yok mu? (liste henüz gelmediyse false: bilinmiyor) */
    fun pcMissing(id: String): Boolean = _ui.value.apps.let { it.isNotEmpty() && it.none { a -> a.id == id } }

    fun clearMessage() = _ui.update { it.copy(message = null) }

    // ---------------------------------------------------------------- komutlar (telefon -> PC -> uygulama)

    /** App launch + T-Link may take 20 seconds; keep a bounded transport allowance. */
    fun send(app: String, name: String, json: String = "{}", cb: ((Boolean, JSONObject?, String?) -> Unit)? = null) {
        val n = Core.node
        if (n == null || !Core.isConnected() || !n.peerSupportsSuite()) { cb?.invoke(false, null, appCtx().getString(R.string.suite_err_offline)); return }
        val req = reqSeq.getAndIncrement()
        if (cb != null) {
            pending.add(req, suiteCommandTimeout(app, name, _ui.value.apps.size), Reply(false, null, appCtx().getString(R.string.suite_err_timeout))) {
                cb(it.ok, it.json, it.error)
            }
        }
        if (!n.sendSuiteCmd(req.toULong(), app, name, json)) {
            pending.complete(req, Reply(false, null, appCtx().getString(R.string.suite_err_offline)))
        }
    }

    /** "disk_full;free=..;needed=.." gibi makine hatalarını kullanıcı metnine çevirir. */
    fun friendlyError(error: String?): String? {
        if (error == "download_unavailable") return ctx.getString(R.string.suite_download_unavailable)
        if (error == "phone_offline") return ctx.getString(R.string.suite_err_offline)
        if (error == "download_not_active" || error == "download_not_paused" || error == "download_not_started") return ctx.getString(R.string.suite_download_control_failed)
        if (error == null || !error.startsWith("disk_full")) return error
        val free = FREE_RE.find(error)?.groupValues?.get(1)?.toLongOrNull()
        return if (free != null) ctx.getString(R.string.suite_err_disk_free, fmtMb(free / 1048576.0)) else ctx.getString(R.string.suite_err_disk)
    }

    fun onReply(req: Long, ok: Boolean, json: String, error: String?) {
        val err = friendlyError(error)
        val parsed = runCatching { JSONObject(json) }.getOrNull()
        handler.post {
            pending.complete(req, Reply(ok, parsed, err))
        }
    }

    fun ramClean(full: Boolean = true) = send("t-ramcash", "ram.clean", """{"mode":"${if (full) "full" else "smart"}"}""") { ok, r, err ->
        val mb = r?.optDouble("freedMb", 0.0) ?: 0.0
        toast(if (ok) ctx.getString(R.string.suite_ram_cleaned, fmtMb(mb)) else (err ?: ctx.getString(R.string.suite_clean_failed)))
    }

    fun refreshDisks() = send("t-desktree", "disk.status") { ok, r, _ ->
        if (!ok || r == null) return@send
        val arr = r.optJSONArray("drives") ?: return@send
        val rows = (0 until arr.length()).map { arr.getJSONObject(it) }.map { DiskRow(it.optString("drive"), it.optLong("freeBytes"), it.optLong("totalBytes")) }
        _ui.update { it.copy(disks = rows) }
    }

    fun updateAll() {
        if (_ui.value.updating) return
        _ui.update { it.copy(updating = true) }
        send("t-bridge", "suite.update") { ok, r, err ->
            _ui.update { it.copy(updating = false) }
            toast(if (ok) (r?.optInt("started", 0) ?: 0).let { ctx.resources.getQuantityString(R.plurals.suite_updating, it, it) } else (err ?: ctx.getString(R.string.suite_update_failed)))
        }
    }

    fun launch(app: String) = send(app, "app.launch") { ok, _, err -> if (!ok) toast(err ?: ctx.getString(R.string.suite_launch_failed)) }

    fun toast(m: String) = _ui.update { it.copy(message = m) }

    // ---------------------------------------------------------------- olaylar (Rust iş parçacığı)

    fun onApps(apps: List<SuiteApp>) = _ui.update { it.copy(apps = apps) }

    fun onEvent(app: String, name: String, json: String) {
        val o = runCatching { JSONObject(json) }.getOrDefault(JSONObject())
        when {
            app == "t-bridge" && name == "file.presented" -> ViewerLink.onPresented(o.optString("name"), o.optBoolean("ok"))
            app == "t-bridge" && name == "file.open" -> ViewerLink.expectOpen(o.optString("name"))
            app == "t-downloader" && (name == "download.progress" || name == "download.state") -> _ui.update {
                val id = o.optString("id")
                it.copy(downloads = it.downloads + (id to DlItem(id, o.optString("name"), o.optDouble("pct"), o.optDouble("speed"), o.optLong("bytes"), o.optLong("total"), o.optString("state") == "paused")))
            }
            app == "t-downloader" && name == "download.completed" -> {
                val d = DlDone(o.optString("id"), o.optString("fileName"), o.optString("path"), o.optLong("size"))
                _ui.update { it.copy(downloads = it.downloads - d.id, completed = (listOf(d) + it.completed.filter { x -> x.id != d.id }).take(10)) }
                notify(ctx.getString(R.string.suite_dl_done), d.name, 1001)
            }
            app == "t-downloader" && name == "download.failed" -> {
                _ui.update { it.copy(downloads = it.downloads - o.optString("id")) }
                notify(ctx.getString(R.string.suite_dl_failed), "${o.optString("name")} · ${o.optString("error")}", 1002)
            }
            name == "ram.status" -> {
                val r = RamInfo(o.optInt("usedPct"), o.optLong("usedMb"), o.optLong("totalMb"))
                _ui.update { it.copy(ram = r) }
                trackRam(r.usedPct)
            }
            name == "ram.cleaned" -> toast(ctx.getString(R.string.suite_ram_cleaned, fmtMb(o.optDouble("freedMb"))))
            name == "disk.low" -> {
                val row = DiskRow(o.optString("drive"), o.optLong("freeBytes"), o.optLong("totalBytes"))
                _ui.update { it.copy(disks = it.disks.filter { d -> d.drive != row.drive } + row) }
                notify(ctx.getString(R.string.suite_disk_low), ctx.getString(R.string.suite_disk_low_text, row.drive, fmtMb(row.freeBytes / 1048576.0)), 1003)
            }
            app !in KNOWN -> _ui.update {
                val line = "$name ${json.take(80)}"
                it.copy(events = it.events + (app to (listOf(line) + (it.events[app] ?: emptyList())).take(5)))
            }
        }
    }

    /** RAM > %90 iki dakikadan uzun sürerse bildirim; 30 dk içinde tekrar etmez. */
    private fun trackRam(pct: Int) {
        val now = SystemClock.elapsedRealtime()
        if (pct <= 90) { ramHighSince = 0; return }
        if (ramHighSince == 0L) ramHighSince = now
        if (now - ramHighSince > 120_000 && (ramLastNotified == 0L || now - ramLastNotified > 30 * 60_000)) {
            ramLastNotified = now
            notify(ctx.getString(R.string.suite_ram_full), ctx.getString(R.string.suite_ram_full_text, pct), 1004)
        }
    }

    @SuppressLint("MissingPermission")
    private fun notify(title: String, text: String, id: Int) {
        if (!_ui.value.notify) return
        val nm = ctx.getSystemService(NotificationManager::class.java)
        if (!nm.areNotificationsEnabled()) return
        nm.notify(id, NotificationCompat.Builder(ctx, CH).setSmallIcon(R.drawable.ic_stat).setContentTitle(title).setContentText(text).setAutoCancel(true).build())
    }

    fun fmtMb(mb: Double): String = if (mb >= 1024) String.format(java.util.Locale.getDefault(), "%.1f GB", mb / 1024.0) else String.format(java.util.Locale.getDefault(), "%.0f MB", mb)
}
