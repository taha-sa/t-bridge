package app.kopru

import android.content.Context
import android.os.Build
import java.io.File

/** Çökme olursa yığın kaydını yerelde saklar; Ayarlar'dan kopyalanabilir. Hiçbir yere gönderilmez. */
object CrashLog {
    private lateinit var file: File

    fun install(ctx: Context) {
        file = File(ctx.filesDir, "last_crash.txt")
        val prev = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { t, e ->
            runCatching {
                file.writeText("T-Bridge ${BuildConfig.VERSION_NAME} · ${Build.MODEL} · Android ${Build.VERSION.RELEASE}\n${t.name}\n${e.stackTraceToString()}")
            }
            prev?.uncaughtException(t, e)
        }
    }

    fun read(): String? = runCatching { file.takeIf { it.exists() }?.readText() }.getOrNull()

    fun clear() {
        runCatching { file.delete() }
    }
}
