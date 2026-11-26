package app.kopru

import android.content.Context
import android.os.Build
import android.os.SystemClock
import android.provider.Settings
import org.json.JSONObject
import java.io.File
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

data class ReportPreview(val text: String, val url: String, val title: String)

/** Hazırlanmış (arındırılmış) hata raporu; GitHub issue bağlantısı 7000 karakterin altında tutulur. */
class CrashReport(
    private val version: String,
    private val utc: String,
    private val os: String,
    private val culture: String,
    private val uptime: String,
    private val type: String,
    private val message: String,
    private val stack: String,
    private val log: List<String>,
    private val sc: Scrubber,
) {
    private fun cell(s: String) = s.replace("|", "\\|").replace('\r', ' ').replace('\n', ' ').replace('`', '\'')

    private fun header() = sc.scrub(
        "# Crash report\n\n| Environment | Value |\n| --- | --- |\n| App | T-Bridge Android |\n| Version | ${cell(version)} |\n| UTC | ${cell(utc)} |\n" +
            "| OS / build | ${cell(os)} |\n| Runtime | Android / Kotlin |\n| Culture | ${cell(culture)} |\n| Uptime | $uptime |\n\n",
    )

    private fun content(): String {
        val t = StringBuilder("$type: $message\n$stack\n--- Recent log (up to 200 lines) ---\n")
        log.forEach { t.append(it).append('\n') }
        return sc.scrub(t.toString()).replace('`', '\'')
    }

    private fun title(): String {
        val ty = type.substringAfterLast('.')
        return sc.scrub("[T-Bridge Android $version] $ty: $message").replace('\r', ' ').replace('\n', ' ').take(180)
    }

    fun preview(): ReportPreview {
        val header = header()
        val content = content()
        val t = title()
        val text = "${header}```text\n$content\n```\n"
        val start = "https://github.com/$REPO/issues/new?title=${esc(t)}&body="
        val head = if (esc(header).length > 2000) "# Crash report\n\nEnvironment omitted (too large); see full preview.\n\n" else header
        fun build(n: Int): String {
            var cut = content.take(n)
            if (cut.isNotEmpty() && cut.last().isHighSurrogate()) cut = cut.dropLast(1)
            val note = if (n < content.length) "\n[truncated; full report available via Copy]" else ""
            return start + esc("$head```text\n$cut$note\n```\n")
        }
        var low = 0
        var high = content.length
        while (low < high) {
            val mid = low + (high - low + 1) / 2
            if (build(mid).length < URL_LIMIT) low = mid else high = mid - 1
        }
        return ReportPreview(text, build(low), t)
    }

    companion object {
        const val REPO = "taha-sa/t-bridge-releases"
        const val URL_LIMIT = 7000

        /** Uri.EscapeDataString ile aynı küme: yalnız A-Z a-z 0-9 - _ . ~ olduğu gibi kalır. */
        fun esc(s: String): String = URLEncoder.encode(s, "UTF-8").replace("+", "%20").replace("*", "%2A").replace("%7E", "~")
    }
}

/**
 * Çökmeleri yerelde saklar ve bir sonraki açılışta kullanıcıya gösterir. Hiçbir şey kendiliğinden gönderilmez:
 * rapor yalnızca kullanıcı "Raporu gönder"e basarsa tarayıcıda (önceden doldurulmuş issue) açılır.
 */
object CrashLog {
    private lateinit var file: File
    private lateinit var appCtx: Context
    private val startedAt = SystemClock.elapsedRealtime()

    private fun prefs() = appCtx.getSharedPreferences("diag", Context.MODE_PRIVATE)

    /** "Hata raporlarını otomatik hazırla" (varsayılan açık). */
    var auto: Boolean
        get() = prefs().getBoolean("auto", true)
        set(v) = prefs().edit().putBoolean("auto", v).apply()

    private fun scrubber(): Scrubber {
        val dev = runCatching { Settings.Global.getString(appCtx.contentResolver, "device_name") }.getOrNull()
        return Scrubber(listOfNotNull(dev))
    }

    private fun utcNow(): String = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }.format(Date())

    private fun uptime(): String {
        val s = (SystemClock.elapsedRealtime() - startedAt) / 1000
        return "%02d:%02d:%02d".format(s / 3600, (s / 60) % 60, s % 60)
    }

    fun install(ctx: Context) {
        appCtx = ctx.applicationContext
        file = File(appCtx.filesDir, "pending_crash.json")
        AppLog.init(appCtx)
        val prev = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { t, e ->
            runCatching {
                if (auto) {
                    val sc = scrubber()
                    val j = JSONObject()
                        .put("utc", utcNow()).put("uptime", uptime()).put("thread", sc.scrub(t.name))
                        .put("type", e.javaClass.name).put("message", sc.scrub(e.message)).put("stack", sc.scrub(e.stackTraceToString()).take(131072))
                    file.writeText(j.toString())
                }
            }
            prev?.uncaughtException(t, e)
        }
    }

    private fun culture() = appCtx.resources.configuration.locales[0].language

    private fun os() = "Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}) · ${Build.MANUFACTURER} ${Build.MODEL}"

    /** Önceki oturumdan kalan çökme raporunun önizlemesi (yoksa null). */
    fun pending(): ReportPreview? = runCatching {
        val j = JSONObject(file.readText())
        CrashReport(
            BuildConfig.VERSION_NAME, j.getString("utc"), os(), culture(), j.optString("uptime", "00:00:00"),
            j.getString("type"), j.optString("message"), j.optString("stack"), AppLog.recent(), scrubber(),
        ).preview()
    }.getOrNull()

    fun dismiss() {
        runCatching { file.delete() }
    }

    /** "Hata raporu gönder…": çökme olmadan, uygulamanın kendi son günlük satırlarından rapor hazırlar. */
    fun manual(): ReportPreview {
        val lines = AppLog.recent()
        return CrashReport(BuildConfig.VERSION_NAME, utcNow(), os(), culture(), uptime(), "UserReport", "Manual report", "", lines, scrubber()).preview()
    }
}
