package app.kopru

import android.app.Notification
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import uniffi.kopru_core.NotificationData
import java.io.ByteArrayOutputStream

/** Bildirimleri PC'ye yansıtır; PC'den kapatılanı telefonda da kapatır. */
class NotifListener : NotificationListenerService() {
    companion object {
        @Volatile
        private var instance: NotifListener? = null
        private val labels = HashMap<String, String>()
        /** Aynı bildirim içeriğini tekrar göndermemek için key → içerik özeti. */
        private val lastSent = HashMap<String, Int>()

        val connected get() = instance != null

        fun dismiss(key: String) {
            instance?.runCatching { cancelNotification(key) }
        }

        fun syncAll() {
            val l = instance ?: return
            synchronized(lastSent) { lastSent.clear() }
            runCatching { l.activeNotifications }.getOrNull()?.forEach { l.forward(it) }
        }

        fun appLabel(ctx: Context, pkg: String): String = synchronized(labels) {
            labels.getOrPut(pkg) {
                runCatching {
                    val pm = ctx.packageManager
                    pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString()
                }.getOrDefault(pkg)
            }
        }
    }

    override fun onListenerConnected() {
        instance = this
        MediaControl.attach(this)
        if (Core.isConnected()) syncAll()
    }

    override fun onListenerDisconnected() {
        instance = null
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) = forward(sbn)

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        synchronized(lastSent) { lastSent.remove(sbn.key) }
        Core.node?.removeNotification(sbn.key)
    }

    private fun forward(sbn: StatusBarNotification) {
        val node = Core.node ?: return
        if (!Core.isConnected()) return
        val n = sbn.notification ?: return
        if (sbn.packageName == packageName) return
        if (sbn.isOngoing || n.flags and Notification.FLAG_GROUP_SUMMARY != 0) return
        if (n.category == Notification.CATEGORY_TRANSPORT || n.category == Notification.CATEGORY_PROGRESS) return
        val ex = n.extras
        val title = ex.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (ex.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: ex.getCharSequence(Notification.EXTRA_TEXT))?.toString().orEmpty()
        if (title.isBlank() && text.isBlank()) return
        val sig = (title + "\u0000" + text).hashCode()
        synchronized(lastSent) {
            if (lastSent[sbn.key] == sig) return
            lastSent[sbn.key] = sig
        }
        val pkg = sbn.packageName
        if (Core.sentIcons.add(pkg)) {
            iconPng(pkg)?.let { node.sendAppIcon(pkg, it) }
        }
        node.sendNotification(
            NotificationData(
                key = sbn.key,
                `package` = pkg,
                appName = appLabel(this, pkg),
                title = title.take(500),
                text = text.take(4000),
                time = sbn.postTime,
            ),
        )
    }

    private fun iconPng(pkg: String): ByteArray? = runCatching {
        val d = packageManager.getApplicationIcon(pkg)
        val size = 64
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val c = Canvas(bmp)
        d.setBounds(0, 0, size, size)
        d.draw(c)
        ByteArrayOutputStream().use { out ->
            bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
            bmp.recycle()
            out.toByteArray()
        }
    }.getOrNull()
}
