package app.kopru

import android.annotation.SuppressLint
import android.content.ContentValues
import android.content.Context
import android.content.SharedPreferences
import android.net.Uri
import android.provider.MediaStore
import android.provider.OpenableColumns
import uniffi.kopru_core.FileSource
import uniffi.kopru_core.OpenedFd

/**
 * Gelen dosyalar MediaStore üzerinden "İndirilenler/Köprü" altına yazılır.
 * Aktarım bitene kadar IS_PENDING=1 (diğer uygulamalara görünmez). Devam için key→uri kalıcıdır.
 */
@SuppressLint("StaticFieldLeak") // yalnızca applicationContext tutulur
object IncomingFiles {
    private lateinit var ctx: Context
    private lateinit var prefs: SharedPreferences
    private const val ROOT = "Download/Köprü"

    fun init(c: Context) {
        ctx = c
        prefs = c.getSharedPreferences("incoming", Context.MODE_PRIVATE)
    }

    private fun sanitize(name: String): List<String> {
        val parts = name.split('/', '\\')
            .filter { it.isNotBlank() && it != "." && it != ".." }
            .map { p -> p.map { c -> if (c.isISOControl() || "<>:\"|?*".indexOf(c) >= 0) '_' else c }.joinToString("").trim().trimEnd('.') }
            .map { it.ifEmpty { "_" } }
        return parts.ifEmpty { listOf("dosya") }
    }

    @Synchronized
    fun open(key: String, name: String, size: Long): OpenedFd {
        val cr = ctx.contentResolver
        prefs.getString(key, null)?.let { saved ->
            val uri = Uri.parse(saved)
            val pfd = runCatching { cr.openFileDescriptor(uri, "rw") }.getOrNull()
            if (pfd != null) {
                val existing = pfd.statSize.coerceAtLeast(0)
                return OpenedFd(pfd.detachFd(), existing.toULong())
            }
            prefs.edit().remove(key).apply()
        }
        val parts = sanitize(name)
        val dir = (listOf(ROOT) + parts.dropLast(1)).joinToString("/") + "/"
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, parts.last())
            put(MediaStore.MediaColumns.RELATIVE_PATH, dir)
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val uri = cr.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: error("İndirilenler'e yazılamadı")
        val pfd = cr.openFileDescriptor(uri, "rw") ?: error("dosya açılamadı")
        prefs.edit().putString(key, uri.toString()).apply()
        return OpenedFd(pfd.detachFd(), 0u)
    }

    @Synchronized
    fun complete(key: String, ok: Boolean): String? {
        val saved = prefs.getString(key, null) ?: return null
        prefs.edit().remove(key).apply()
        val uri = Uri.parse(saved)
        val cr = ctx.contentResolver
        return if (ok) {
            cr.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
            // Görünen yol: İndirilenler/Köprü/…/ad
            cr.query(uri, arrayOf(MediaStore.MediaColumns.RELATIVE_PATH, MediaStore.MediaColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) "${c.getString(0)}${c.getString(1)}" else null
            } ?: uri.toString()
        } else {
            runCatching { cr.delete(uri, null, null) }
            null
        }
    }

    /** Tamamlanan dosyanın içerik URI'si (açmak için). */
    fun findByPath(path: String): Uri? {
        val slash = path.lastIndexOf('/')
        if (slash < 0) return null
        val rel = path.substring(0, slash + 1)
        val name = path.substring(slash + 1)
        return ctx.contentResolver.query(
            MediaStore.Downloads.EXTERNAL_CONTENT_URI,
            arrayOf(MediaStore.MediaColumns._ID),
            "${MediaStore.MediaColumns.RELATIVE_PATH}=? AND ${MediaStore.MediaColumns.DISPLAY_NAME}=?",
            arrayOf(rel, name),
            null,
        )?.use { c -> if (c.moveToFirst()) Uri.withAppendedPath(MediaStore.Downloads.EXTERNAL_CONTENT_URI, c.getLong(0).toString()) else null }
    }
}

object OutgoingFiles {
    fun openFd(ctx: Context, source: String): Int {
        val pfd = ctx.contentResolver.openFileDescriptor(Uri.parse(source), "r") ?: error("dosya açılamadı")
        return pfd.detachFd()
    }

    /** URI'den ad + boyut okur. Seçiciden gelen URI'ler için kalıcı okuma izni alınır (kuyruk yeniden başlatmada sürsün). */
    fun describe(ctx: Context, uri: Uri, persist: Boolean): FileSource? {
        val cr = ctx.contentResolver
        if (persist) runCatching { cr.takePersistableUriPermission(uri, android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION) }
        var name: String? = null
        var size = -1L
        runCatching {
            cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    name = c.getString(0)
                    if (!c.isNull(1)) size = c.getLong(1)
                }
            }
        }
        if (size < 0) {
            size = runCatching { cr.openFileDescriptor(uri, "r")?.use { it.statSize } }.getOrNull() ?: -1L
        }
        if (size < 0) return null
        return FileSource(uri.toString(), name ?: uri.lastPathSegment ?: "dosya", size.toULong())
    }
}
