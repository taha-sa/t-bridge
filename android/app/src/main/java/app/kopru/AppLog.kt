package app.kopru

import android.content.Context
import android.provider.Settings
import android.util.Log
import java.io.File
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit

/** Private, scrubbed, bounded logs. Disk writes never run on the caller's thread. */
object AppLog {
    private const val MAX_BYTES = 256 * 1024
    private val gate = Any()
    @Volatile private var directory: File? = null
    @Volatile private var scrubber = Scrubber(emptyList())
    private val writer = ThreadPoolExecutor(
        1, 1, 0L, TimeUnit.MILLISECONDS, ArrayBlockingQueue<Runnable>(256),
        { r -> Thread(r, "kopru-log").apply { isDaemon = true } }, ThreadPoolExecutor.DiscardOldestPolicy(),
    )

    fun init(context: Context) {
        directory = File(context.applicationContext.filesDir, "logs")
        val device = runCatching { Settings.Global.getString(context.contentResolver, "device_name") }.getOrNull()
        scrubber = Scrubber(listOfNotNull(device))
        i("kopru", "Application starting")
    }

    private fun write(priority: Int, tag: String, message: String, error: Throwable?): Int {
        val clean = scrubber.scrub(message + (error?.let { "\n" + it.stackTraceToString() } ?: "")).take(16384)
        val safeTag = scrubber.scrub(tag).replace('\n', ' ').replace('\r', ' ').take(80)
        val result = Log.println(priority, safeTag, clean)
        val folder = directory ?: return result
        writer.execute {
            runCatching {
                synchronized(gate) {
                    folder.mkdirs()
                    val file = File(folder, "app.log")
                    if (file.length() >= MAX_BYTES) {
                        File(folder, "app.log.2").delete()
                        File(folder, "app.log.1").renameTo(File(folder, "app.log.2"))
                        file.renameTo(File(folder, "app.log.1"))
                        // If rotation failed, keep storage bounded rather than append indefinitely.
                        if (file.exists()) file.writeText("")
                    }
                    file.appendText("${System.currentTimeMillis()} $priority/$safeTag: $clean\n")
                }
            }
        }
        return result
    }

    fun recent(limit: Int = 200): List<String> = synchronized(gate) {
        val folder = directory ?: return@synchronized emptyList()
        val lines = ArrayDeque<String>()
        for (name in listOf("app.log.2", "app.log.1", "app.log")) {
            runCatching { File(folder, name).forEachLine { line ->
                lines.addLast(line)
                while (lines.size > limit.coerceIn(1, 200)) lines.removeFirst()
            } }
        }
        lines.toList()
    }

    fun v(tag: String, message: String, error: Throwable? = null) = write(Log.VERBOSE, tag, message, error)
    fun d(tag: String, message: String, error: Throwable? = null) = write(Log.DEBUG, tag, message, error)
    fun i(tag: String, message: String, error: Throwable? = null) = write(Log.INFO, tag, message, error)
    fun w(tag: String, message: String, error: Throwable? = null) = write(Log.WARN, tag, message, error)
    fun e(tag: String, message: String, error: Throwable? = null) = write(Log.ERROR, tag, message, error)
}
