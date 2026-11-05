package app.kopru

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.widget.Toast
import androidx.core.content.FileProvider
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.Executors

/**
 * Kendi kendini güncelleme: GitHub Releases'teki `android.json` denetlenir, yeni APK önbelleğe iner,
 * SHA-256 doğrulanır, "Yükle" bildirimi çıkar. Kurulum Android'in yükleyicisiyle olur; Android 11'de
 * sistem bir kez onay ister (atlanamaz). Aynı imza anahtarı şart olduğundan veri ve eşleşme korunur.
 */
@SuppressLint("StaticFieldLeak") // yalnızca applicationContext
object Updater {
    private const val TAG = "kopru-upd"
    const val MANIFEST_URL = "https://github.com/taha-sa/t-bridge-releases/releases/latest/download/android.json"
    private const val CH_UPDATE = "update"
    private const val ID_UPDATE = 3
    private const val DAY = 24 * 60 * 60 * 1000L

    data class Info(val versionCode: Int, val versionName: String, val url: String, val sha256: String, val notes: String)

    sealed interface State {
        data object Idle : State
        data object Checking : State
        data object Latest : State
        data class Downloading(val info: Info, val progress: Float) : State
        data class Ready(val info: Info) : State
        data object Installing : State
        data class Error(val message: String) : State
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state.asStateFlow()

    private val io = Executors.newSingleThreadExecutor { Thread(it, "kopru-update").apply { isDaemon = true } }
    private val main = Handler(Looper.getMainLooper())
    private lateinit var ctx: Context
    private val prefs by lazy { ctx.getSharedPreferences("update", Context.MODE_PRIVATE) }

    fun init(c: Context) {
        ctx = c.applicationContext
    }

    var auto: Boolean
        get() = prefs.getBoolean("auto", true)
        set(v) = prefs.edit().putBoolean("auto", v).apply()

    private fun apk() = File(ctx.cacheDir, "update.apk")

    /** Servis başlarken çağrılır: kısa gecikmeyle ve günde bir, yalnızca ölçülmeyen ağda (Wi-Fi). */
    fun schedule() {
        main.removeCallbacks(tick)
        main.postDelayed(tick, 60_000)
    }

    private val tick = object : Runnable {
        override fun run() {
            if (auto && unmetered()) check(manual = false)
            main.postDelayed(this, DAY)
        }
    }

    private fun unmetered(): Boolean {
        val cm = ctx.getSystemService(ConnectivityManager::class.java) ?: return false
        val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return false
        return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
    }

    fun check(manual: Boolean) {
        val s = _state.value
        if (s is State.Checking || s is State.Downloading || s is State.Installing) return
        if (s is State.Ready && apk().exists()) {
            if (!manual) notifyReady(s.info)
            return
        }
        _state.value = State.Checking
        io.execute {
            try {
                val info = fetchManifest()
                if (info.versionCode <= BuildConfig.VERSION_CODE) {
                    apk().delete()
                    _state.value = State.Latest
                    return@execute
                }
                download(info)
                val ready = State.Ready(info)
                lastReady = ready
                _state.value = ready
                notifyReady(info)
            } catch (e: Exception) {
                Log.w(TAG, "güncelleme denetlenemedi", e)
                _state.value = State.Error(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    private fun open(url: String): HttpURLConnection {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = 15_000
        c.readTimeout = 30_000
        c.instanceFollowRedirects = true
        c.setRequestProperty("User-Agent", "T-Bridge/${BuildConfig.VERSION_NAME}")
        c.setRequestProperty("Accept", "application/octet-stream, application/json")
        return c
    }

    private fun fetchManifest(): Info {
        val c = open(MANIFEST_URL)
        try {
            if (c.responseCode == 404) throw IllegalStateException("henüz yayınlanmış sürüm yok")
            if (c.responseCode !in 200..299) throw IllegalStateException("sunucu yanıtı ${c.responseCode}")
            val j = JSONObject(c.inputStream.bufferedReader().use { it.readText() })
            val url = j.getString("url")
            require(url.startsWith("https://")) { "güvensiz adres" }
            return Info(j.getInt("versionCode"), j.getString("versionName"), url, j.getString("sha256").lowercase(), j.optString("notes"))
        } finally {
            c.disconnect()
        }
    }

    private fun download(info: Info) {
        val out = apk()
        // Önceden inmiş ve doğrulanmış aynı dosya varsa tekrar indirme.
        if (out.exists() && sha256(out) == info.sha256) return
        val tmp = File(ctx.cacheDir, "update.part")
        val c = open(info.url)
        try {
            if (c.responseCode !in 200..299) throw IllegalStateException("indirme yanıtı ${c.responseCode}")
            val total = c.contentLengthLong
            val md = MessageDigest.getInstance("SHA-256")
            var got = 0L
            var lastEmit = 0L
            c.inputStream.use { input ->
                tmp.outputStream().use { o ->
                    val buf = ByteArray(64 * 1024)
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        o.write(buf, 0, n)
                        md.update(buf, 0, n)
                        got += n
                        val now = System.currentTimeMillis()
                        if (now - lastEmit > 300 && total > 0) {
                            lastEmit = now
                            _state.value = State.Downloading(info, got.toFloat() / total)
                        }
                    }
                }
            }
            val hash = md.digest().joinToString("") { "%02x".format(it) }
            if (hash != info.sha256) {
                tmp.delete()
                throw SecurityException("indirilen dosya doğrulanamadı")
            }
            out.delete()
            if (!tmp.renameTo(out)) throw IllegalStateException("dosya taşınamadı")
        } finally {
            c.disconnect()
        }
    }

    private fun sha256(f: File): String {
        val md = MessageDigest.getInstance("SHA-256")
        f.inputStream().use { i ->
            val b = ByteArray(64 * 1024)
            while (true) {
                val n = i.read(b)
                if (n < 0) break
                md.update(b, 0, n)
            }
        }
        return md.digest().joinToString("") { "%02x".format(it) }
    }

    private fun notifyReady(info: Info) {
        val nm = ctx.getSystemService(NotificationManager::class.java) ?: return
        nm.createNotificationChannel(NotificationChannel(CH_UPDATE, "Güncellemeler", NotificationManager.IMPORTANCE_DEFAULT))
        val pi = PendingIntent.getActivity(
            ctx, 7, Intent(ctx, UpdateActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val n = Notification.Builder(ctx, CH_UPDATE)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle("T-Bridge ${info.versionName} hazır")
            .setContentText(info.notes.ifBlank { "Yüklemek için dokun — eşleşme ve ayarlar korunur." })
            .setContentIntent(pi)
            .setAutoCancel(true)
            .addAction(Notification.Action.Builder(null, "Yükle", pi).build())
            .build()
        nm.notify(ID_UPDATE, n)
    }

    fun canInstall(): Boolean = ctx.packageManager.canRequestPackageInstalls()

    fun openInstallPermission(a: Activity) {
        a.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${a.packageName}")))
    }

    /** Ön plandaki bir etkinlikten çağrılır (onay ekranı arka plandan açılamaz). */
    fun install(a: Activity) {
        val s = _state.value
        if (s !is State.Ready || !apk().exists()) {
            Toast.makeText(a, "Hazır güncelleme yok", Toast.LENGTH_SHORT).show()
            return
        }
        if (!canInstall()) {
            Toast.makeText(a, "T-Bridge'e bu kaynaktan yükleme izni ver, sonra tekrar dene.", Toast.LENGTH_LONG).show()
            openInstallPermission(a)
            return
        }
        ctx.getSystemService(NotificationManager::class.java)?.cancel(ID_UPDATE)
        _state.value = State.Installing
        try {
            val pi = ctx.packageManager.packageInstaller
            val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                setAppPackageName(ctx.packageName)
                if (Build.VERSION.SDK_INT >= 31) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
            }
            val id = pi.createSession(params)
            pi.openSession(id).use { session ->
                session.openWrite("base.apk", 0, apk().length()).use { o ->
                    apk().inputStream().use { it.copyTo(o) }
                    session.fsync(o)
                }
                val cb = PendingIntent.getActivity(
                    ctx, 8, Intent(ctx, UpdateActivity::class.java).setAction(UpdateActivity.ACTION_STATUS),
                    PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
                session.commit(cb.intentSender)
            }
        } catch (e: Exception) {
            Log.w(TAG, "oturumla yükleme başarısız, sistem yükleyicisi deneniyor", e)
            fallback(a)
        }
    }

    /** MIUI vb. oturumu engellerse: APK'yı sistem yükleyicisine ver. */
    fun fallback(a: Activity) {
        val uri = FileProvider.getUriForFile(a, "${a.packageName}.files", apk())
        @Suppress("DEPRECATION")
        val i = Intent(Intent.ACTION_INSTALL_PACKAGE).setData(uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            .putExtra(Intent.EXTRA_NOT_UNKNOWN_SOURCE, true)
        runCatching { a.startActivity(i) }.onFailure {
            _state.value = State.Error("yükleyici açılamadı: ${it.message}")
            return
        }
        // Yükleyici açıldı; vazgeçilirse paket hazır kalsın.
        _state.value = lastReady ?: State.Idle
    }

    internal fun onStatus(a: Activity, intent: Intent) {
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                if (confirm != null) a.startActivity(confirm)
            }
            PackageInstaller.STATUS_SUCCESS -> {
                // Normalde buraya gelinmeden süreç yeni sürümle yeniden başlar.
                _state.value = State.Latest
            }
            PackageInstaller.STATUS_FAILURE_ABORTED -> {
                // Kullanıcı vazgeçti: paket hazır kalsın.
                _state.value = lastReady ?: State.Idle
            }
            else -> {
                val msg = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "bilinmeyen hata ($status)"
                Log.w(TAG, "yükleme başarısız: $msg")
                fallback(a)
            }
        }
    }

    /** Son hazır paket (yükleme iptal edilirse "hazır" durumuna dönmek için). */
    @Volatile
    private var lastReady: State.Ready? = null

    internal fun rememberReady() {
        (state.value as? State.Ready)?.let { lastReady = it }
    }
}

/** Görünmez etkinlik: bildirime dokununca yüklemeyi başlatır, yükleyici sonucunu alır. */
class UpdateActivity : Activity() {
    companion object {
        const val ACTION_STATUS = "app.kopru.UPDATE_STATUS"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Updater.init(this)
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    private fun handle(i: Intent?) {
        if (i?.action == ACTION_STATUS) {
            Updater.onStatus(this, i)
        } else {
            Updater.rememberReady()
            Updater.install(this)
        }
        finish()
    }
}
