package app.kopru

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.os.BatteryManager
import android.os.IBinder
import android.os.PowerManager
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.TransferInfo
import uniffi.kopru_core.TransferState

/**
 * Ön plan servisi: çekirdeği canlı tutar, ağ değişimlerini bildirir, pili gönderir.
 * Pil dostu: bağlıyken mDNS kapalı (MulticastLock yok); yalnızca aktarım sırasında Wi-Fi/CPU kilidi.
 */
class KopruService : Service() {
    @SuppressLint("StaticFieldLeak") // yalnızca applicationContext tutulur
    companion object {
        const val CH_STATUS = "status"
        const val CH_EVENTS = "events"
        const val CH_RING = "ring"
        const val ID_STATUS = 1
        const val ID_RING = 2
        const val ID_FILE = 1000

        @Volatile
        var appCtx: Context? = null

        @Volatile
        private var instance: KopruService? = null

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, KopruService::class.java))
        }

        /**
         * Paylaş menüsünden gelen URI izni etkinlik kapanınca düşer. Aynı URI'leri izinle servise de
         * vererek izni servisin ömrüne bağlarız: bilgisayar sonradan bağlansa da kuyruktaki dosya okunabilir.
         */
        fun startHolding(ctx: Context, uris: List<android.net.Uri>) {
            if (uris.isEmpty()) return start(ctx)
            val clip = android.content.ClipData.newRawUri("kopru", uris[0])
            uris.drop(1).forEach { clip.addItem(android.content.ClipData.Item(it)) }
            val i = Intent(ctx, KopruService::class.java).apply {
                clipData = clip
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            ctx.startForegroundService(i)
        }

        /** mDNS tarama penceresi için MulticastLock tut/bırak (Core.rescan / Scanning(false) olayı). */
        fun holdMulticast(on: Boolean) {
            instance?.updateMulticast(on)
        }

        fun onConnState(state: ConnState) {
            instance?.apply {
                if (state == ConnState.CONNECTED) updateMulticast(false)   // bağlıyken mDNS kilidi gerekmez
                updateNotification()
                if (state == ConnState.CONNECTED) {
                    lastBattery = null
                    sendBattery()
                    MediaControl.publish(force = true)
                    NotifListener.syncAll()
                }
            }
        }

        fun onTransfer(t: TransferInfo) {
            instance?.apply {
                updateLocks()
                if (t.state == TransferState.DONE && t.direction == uniffi.kopru_core.Direction.INCOMING) notifyReceived(t)
            }
        }

        fun showRinging(ctx: Context, on: Boolean) {
            val nm = ctx.getSystemService(NotificationManager::class.java)
            if (!on) {
                nm.cancel(ID_RING)
                return
            }
            val stop = PendingIntent.getBroadcast(
                ctx, 1, Intent(ctx, ActionReceiver::class.java).setAction(ActionReceiver.STOP_RING),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            val n = Notification.Builder(ctx, CH_RING)
                .setSmallIcon(R.drawable.ic_stat)
                .setContentTitle("Telefon çalıyor")
                .setContentText("Bilgisayardan çaldırıldı. Durdurmak için dokun.")
                .setContentIntent(stop)
                .addAction(Notification.Action.Builder(null, "Durdur", stop).build())
                .setOngoing(true)
                .setCategory(Notification.CATEGORY_ALARM)
                .build()
            nm.notify(ID_RING, n)
        }
    }

    private var multicast: WifiManager.MulticastLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var lastBattery: Pair<Int, Boolean>? = null

    private val batteryReceiver = object : BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) = sendBattery(i)
    }

    private val netCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            // Yeni ağda eski tarama soketleri işe yaramaz: taramayı baştan başlat.
            Core.node?.rescan()
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        appCtx = applicationContext
        instance = this
        createChannels()
        startForeground(ID_STATUS, buildNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        Core.start()
        registerReceiver(batteryReceiver, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val cm = getSystemService(ConnectivityManager::class.java)
        cm.registerNetworkCallback(
            NetworkRequest.Builder().addTransportType(NetworkCapabilities.TRANSPORT_WIFI).build(),
            netCallback,
        )
        updateMulticast(!Core.isConnected())
        updateNotification()
        Updater.schedule()
        // Etkinlik çekirdeği servisten önce başlatmış olabilir: kilitsiz gönderilen ilk tarama yanıtları
        // telefonda düşer. Kilit alındıktan sonra taramayı tekrarla.
        if (!Core.isConnected()) Core.node?.rescan()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Core.start()
        return START_STICKY
    }

    override fun onDestroy() {
        instance = null
        runCatching { unregisterReceiver(batteryReceiver) }
        runCatching { getSystemService(ConnectivityManager::class.java).unregisterNetworkCallback(netCallback) }
        updateMulticast(false)
        releaseLocks()
        super.onDestroy()
    }

    private fun createChannels() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CH_STATUS, "Bağlantı durumu", NotificationManager.IMPORTANCE_MIN).apply {
            setShowBadge(false)
        })
        nm.createNotificationChannel(NotificationChannel(CH_EVENTS, "Alınan dosyalar", NotificationManager.IMPORTANCE_DEFAULT))
        nm.createNotificationChannel(NotificationChannel(CH_RING, "Telefonu bul", NotificationManager.IMPORTANCE_HIGH).apply {
            setSound(null, null)
        })
    }

    private fun buildNotification(): Notification {
        val ui = Core.ui.value
        val text = when (ui.conn) {
            ConnState.CONNECTED -> "${ui.peer?.name ?: "Bilgisayar"} bağlı"
            ConnState.CONNECTING -> "Bağlanıyor…"
            ConnState.DISCONNECTED -> if (ui.peers.isEmpty()) "Eşleşmiş bilgisayar yok" else "Bilgisayar bekleniyor"
        }
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val clip = PendingIntent.getActivity(
            this, 2, ClipReadActivity.intent(this), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return Notification.Builder(this, CH_STATUS)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle("T-Bridge")
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setShowWhen(false)
            .addAction(Notification.Action.Builder(null, "Panoyu PC'ye gönder", clip).build())
            .build()
    }

    fun updateNotification() {
        getSystemService(NotificationManager::class.java).notify(ID_STATUS, buildNotification())
    }

    @SuppressLint("WakelockTimeout")
    fun updateMulticast(on: Boolean) {
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        if (on && multicast == null) {
            multicast = wm.createMulticastLock("kopru-mdns").apply { setReferenceCounted(false); acquire() }
        } else if (!on) {
            multicast?.runCatching { if (isHeld) release() }
            multicast = null
        }
    }

    /** Aktarım sürerken ekran kapalı olsa da Wi-Fi tam hızda, CPU uyanık kalır. */
    @SuppressLint("WakelockTimeout")
    fun updateLocks() {
        val active = Core.activeTransfers()
        if (active && wakeLock == null) {
            val wm = applicationContext.getSystemService(WifiManager::class.java)
            @Suppress("DEPRECATION")
            wifiLock = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "kopru-transfer").apply { setReferenceCounted(false); acquire() }
            wakeLock = getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "kopru:transfer").apply { setReferenceCounted(false); acquire(6 * 60 * 60 * 1000L) }
        } else if (!active) {
            releaseLocks()
        }
    }

    private fun releaseLocks() {
        wifiLock?.runCatching { if (isHeld) release() }
        wakeLock?.runCatching { if (isHeld) release() }
        wifiLock = null
        wakeLock = null
    }

    fun sendBattery(intent: Intent? = registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))) {
        val i = intent ?: return
        val level = i.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
        val scale = i.getIntExtra(BatteryManager.EXTRA_SCALE, 100)
        if (level < 0 || scale <= 0) return
        val pct = (level * 100 / scale).coerceIn(0, 100)
        val status = i.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
        val charging = status == BatteryManager.BATTERY_STATUS_CHARGING || status == BatteryManager.BATTERY_STATUS_FULL
        val cur = pct to charging
        if (cur == lastBattery) return
        if (Core.node?.sendBattery(pct.toUByte(), charging) == true) lastBattery = cur
    }

    private fun notifyReceived(t: TransferInfo) {
        val path = t.detail ?: return
        val uri = IncomingFiles.findByPath(path)
        val name = t.name.substringAfterLast('/')
        val autoOpen = uri != null && ViewerLink.consumeOpen(name)
        if (autoOpen && uri != null) runCatching { ViewerLink.open(this, uri, name) }   // arka plandaysa Android engelleyebilir: bildirim yedek
        val view = uri?.let {
            val i = if (autoOpen) ViewerLink.openIntent(this, it, name) else Intent(Intent.ACTION_VIEW).setDataAndType(it, contentResolver.getType(it) ?: "*/*")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            PendingIntent.getActivity(this, (t.id % 100000uL).toInt(), i, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
        val n = Notification.Builder(this, CH_EVENTS)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle(if (autoOpen) "Dosya alındı · dokunarak aç" else "Dosya alındı")
            .setContentText(name)
            .setAutoCancel(true)
            .apply { if (view != null) setContentIntent(view) }
            .build()
        getSystemService(NotificationManager::class.java).notify(ID_FILE + (t.id % 100000uL).toInt(), n)
    }
}
