package app.kopru

import android.app.Notification
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
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

        /** PC'den yanıtlanabilen bildirimin gerekli parçaları. */
        private class ReplyHandle(val pi: PendingIntent, val inputs: Array<RemoteInput>, val resultKey: String)

        /** key → yanıt eylemi. Üst sınır ~100 (en eski düşer); bildirim kaldırılınca silinir. */
        private val replies = object : LinkedHashMap<String, ReplyHandle>(128, 0.75f, false) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, ReplyHandle>?) = size > 100
        }

        /** SEMANTIC_ACTION_REPLY tercih edilir; yoksa serbest metin girişi olan ilk RemoteInput'lu eylem (WearableExtender dahil). */
        private fun findReply(n: Notification): Pair<Notification.Action, RemoteInput>? {
            val allActions = buildList {
                n.actions?.let { addAll(it) }
                runCatching { Notification.WearableExtender(n).actions }.getOrNull()?.let { addAll(it) }
            }
            if (allActions.isEmpty()) return null
            fun freeForm(a: Notification.Action) = a.remoteInputs?.firstOrNull { it.allowFreeFormInput }
            if (Build.VERSION.SDK_INT >= 28) {
                for (a in allActions) {
                    if (a.semanticAction == Notification.Action.SEMANTIC_ACTION_REPLY && a.actionIntent != null) {
                        val ri = freeForm(a) ?: a.remoteInputs?.firstOrNull()
                        if (ri != null) return a to ri
                    }
                }
            }
            for (a in allActions) {
                if (a.actionIntent == null) continue
                val ri = freeForm(a) ?: continue
                return a to ri
            }
            return null
        }

        /** PC'den gelen yanıtı kaynak uygulamanın RemoteInput'una doldurup PendingIntent'i gönderir. */
        fun reply(ctx: Context, key: String, text: String) {
            val node = Core.node ?: return
            fun result(ok: Boolean, err: String?) {
                node.sendNotificationReplyResult(key, ok, err)
            }
            val h = synchronized(replies) { replies[key] }
            if (h == null) { result(false, ctx.getString(R.string.notif_err_invalid)); return }
            try {
                val intent = Intent().addFlags(Intent.FLAG_RECEIVER_FOREGROUND)
                val bundle = Bundle().apply { putCharSequence(h.resultKey, text) }
                RemoteInput.addResultsToIntent(h.inputs, intent, bundle)
                if (Build.VERSION.SDK_INT >= 28) RemoteInput.setResultsSource(intent, RemoteInput.SOURCE_FREE_FORM_INPUT)
                h.pi.send(ctx, 0, intent)
                result(true, null)
            } catch (e: PendingIntent.CanceledException) {
                synchronized(replies) { replies.remove(key) }
                result(false, ctx.getString(R.string.notif_err_invalid))
            } catch (e: Exception) {
                result(false, e.message ?: ctx.getString(R.string.notif_err_reply_failed))
            }
        }

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
        synchronized(replies) { replies.remove(sbn.key) }
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
        val msgText = runCatching {
            @Suppress("DEPRECATION")
            val arr = ex.getParcelableArray(Notification.EXTRA_MESSAGES)
            val last = arr?.lastOrNull() as? Bundle
            val body = last?.getCharSequence("text")?.toString().orEmpty()
            val sender = last?.getCharSequence("sender")?.toString().orEmpty()
            when {
                body.isBlank() -> ""
                sender.isNotBlank() && !title.contains(sender) -> "$sender: $body"
                else -> body
            }
        }.getOrDefault("")
        val text = (ex.getCharSequence(Notification.EXTRA_BIG_TEXT)
            ?: ex.getCharSequence(Notification.EXTRA_TEXT))
            ?.toString().orEmpty().ifBlank { msgText }
        if (title.isBlank() && text.isBlank()) return
        // Yanıt eylemi her güncellemede tazelenir (PendingIntent yenilenmiş olabilir).
        val reply = findReply(n)
        synchronized(replies) {
            if (reply != null) {
                replies[sbn.key] = ReplyHandle(reply.first.actionIntent, arrayOf(reply.second), reply.second.resultKey)
            } else {
                replies.remove(sbn.key)
            }
        }
        val sig = (title + "\u0000" + text + "\u0000" + (reply != null)).hashCode()
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
        // Yeni sürüm PC'ye yanıtlanabilirliği bildir (eski PC bu yeteneği duyurmaz: çekirdek göndermez).
        if (reply != null) {
            val choices = reply.second.choices?.map { it.toString() }?.filter { it.isNotBlank() }?.take(3).orEmpty()
            node.sendNotificationExt(sbn.key, true, choices)
        }
    }

    private fun iconPng(pkg: String): ByteArray? = runCatching {
        val d = packageManager.getApplicationIcon(pkg)
        val size = 64
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        try {
            val c = Canvas(bmp)
            d.setBounds(0, 0, size, size)
            d.draw(c)
            ByteArrayOutputStream().use { out ->
                bmp.compress(Bitmap.CompressFormat.PNG, 100, out)
                out.toByteArray()
            }
        } finally {
            bmp.recycle()
        }
    }.getOrNull()
}
