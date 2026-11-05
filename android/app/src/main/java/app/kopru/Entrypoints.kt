package app.kopru

import android.annotation.SuppressLint
import android.app.Activity
import android.app.Application
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.service.quicksettings.TileService
import android.widget.RemoteViews
import android.widget.Toast

class KopruApp : Application() {
    override fun onCreate() {
        super.onCreate()
        CrashLog.install(this)
        Core.init(this)
        app.kopru.ui.ThemePref.init(this)
        Updater.init(this)
        KopruService.appCtx = this
    }
}

/**
 * Android 10+ arka planda pano okumayı yasaklar; yalnızca odaklı uygulama okuyabilir.
 * Bu görünmez etkinlik bir an odak alır, panoyu okuyup PC'ye gönderir ve kapanır.
 */
class ClipReadActivity : Activity() {
    companion object {
        fun intent(ctx: Context) = Intent(ctx, ClipReadActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION or Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
    }

    private var done = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        KopruService.start(this)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || done) return
        done = true
        val cm = getSystemService(ClipboardManager::class.java)
        val text = cm?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString()
        val msg = when {
            text.isNullOrEmpty() -> "Panoda metin yok"
            Core.node == null || !Core.isConnected() -> "Bilgisayar bağlı değil"
            Core.sendClipboardText(text) -> "Panoya gönderildi"
            else -> "Zaten gönderildi"
        }
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
        finish()
        @Suppress("DEPRECATION")
        overridePendingTransition(0, 0)
    }
}

/** Hızlı ayarlar kutucuğu: tek dokunuşla panoyu PC'ye gönder. */
class ClipTileService : TileService() {
    override fun onClick() {
        super.onClick()
        val i = ClipReadActivity.intent(this)
        if (Build.VERSION.SDK_INT >= 34) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 3, i, PendingIntent.FLAG_IMMUTABLE))
        } else {
            startLegacy(i)
        }
    }
}

@SuppressLint("StartActivityAndCollapseDeprecated")
@Suppress("DEPRECATION")
private fun TileService.startLegacy(i: Intent) = startActivityAndCollapse(i)

/** Ana ekran widget'ı: tek dokunuşla panoyu PC'ye gönder. */
class ClipWidget : AppWidgetProvider() {
    override fun onUpdate(ctx: Context, mgr: AppWidgetManager, ids: IntArray) {
        val pi = PendingIntent.getActivity(ctx, 4, ClipReadActivity.intent(ctx), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val views = RemoteViews(ctx.packageName, R.layout.widget_clip).apply { setOnClickPendingIntent(R.id.widget_root, pi) }
        mgr.updateAppWidget(ids, views)
    }
}

/** Paylaş menüsünden gelen dosyaları/metni PC'ye gönderir. */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Core.start()
        val uris = mutableListOf<Uri>()
        when (intent?.action) {
            Intent.ACTION_SEND -> {
                @Suppress("DEPRECATION")
                (intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))?.let(uris::add)
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                @Suppress("DEPRECATION")
                intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.let(uris::addAll)
            }
        }
        KopruService.startHolding(this, uris)
        val msg = if (uris.isNotEmpty()) {
            // Kalıcı izin verilmişse al (çoğu uygulama vermez; o durumda izni servis tutar).
            val files = uris.mapNotNull { OutgoingFiles.describe(this, it, persist = true) }
            val n = Core.sendFiles(files)
            when {
                n == 0 -> "Gönderilemedi (eşleşmiş bilgisayar yok?)"
                Core.isConnected() -> "$n dosya gönderiliyor"
                else -> "$n dosya sıraya alındı; bilgisayar bağlanınca gidecek"
            }
        } else {
            val text = intent?.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
            when {
                text.isNullOrEmpty() -> "Paylaşılacak bir şey yok"
                !Core.isConnected() -> "Bilgisayar bağlı değil"
                Core.sendClipboardText(text) -> "Metin PC panosuna gönderildi"
                else -> "Zaten gönderildi"
            }
        }
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
        finish()
    }
}

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
            runCatching { KopruService.start(ctx) }
        }
    }
}

class ActionReceiver : BroadcastReceiver() {
    companion object {
        const val STOP_RING = "app.kopru.STOP_RING"
    }

    override fun onReceive(ctx: Context, intent: Intent) {
        if (intent.action == STOP_RING) Ringer.stop()
    }
}
