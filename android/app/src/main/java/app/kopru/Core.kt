package app.kopru

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import app.kopru.AppLog as Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import uniffi.kopru_core.ConnState
import uniffi.kopru_core.Direction
import uniffi.kopru_core.Event
import uniffi.kopru_core.FileSource
import uniffi.kopru_core.Host
import uniffi.kopru_core.KopruException
import uniffi.kopru_core.Node
import uniffi.kopru_core.NodeConfig
import uniffi.kopru_core.OpenedFd
import uniffi.kopru_core.PeerInfo
import uniffi.kopru_core.PhoneCommand
import uniffi.kopru_core.Platform
import uniffi.kopru_core.Role
import uniffi.kopru_core.ServerInfo
import uniffi.kopru_core.TransferInfo
import uniffi.kopru_core.TransferState
import uniffi.kopru_core.initLogging
import uniffi.kopru_core.setCameraCapability

/** Arayüzün izlediği durum. */
data class UiState(
    val conn: ConnState = ConnState.DISCONNECTED,
    val peer: PeerInfo? = null,
    val peers: List<PeerInfo> = emptyList(),
    val discovered: List<ServerInfo> = emptyList(),
    /** Sınırlı mDNS tarama penceresi açık mı (yalnızca metin/kilit için; animasyon yok). */
    val scanning: Boolean = false,
    val transfers: Map<String, TransferInfo> = emptyMap(),
    val pairError: String? = null,
    val startError: String? = null,
)

/**
 * Çekirdek düğümünün tek örneği ve Host arayüzü. Servis başlatır; etkinlikler buradan okur.
 * Host geri çağrıları Rust iş parçacıklarından gelir: kısa tutulur, UI işleri ana iş parçacığına atılır.
 */
@SuppressLint("StaticFieldLeak") // yalnızca applicationContext tutulur
object Core : Host {
    private const val TAG = "kopru"
    private val main = Handler(Looper.getMainLooper())
    private val replyExec = java.util.concurrent.Executors.newSingleThreadExecutor()
    private lateinit var ctx: Context

    @Volatile
    var node: Node? = null
        private set

    private val _ui = MutableStateFlow(UiState())
    val ui: StateFlow<UiState> = _ui.asStateFlow()

    /** Bu bağlantıda simgesi gönderilmiş paketler. */
    val sentIcons: MutableSet<String> = java.util.Collections.synchronizedSet(HashSet())

    fun init(context: Context) {
        ctx = context.applicationContext
        Suite.init(ctx)
        IncomingFiles.init(ctx)
    }

    @Synchronized
    fun start(): Node? {
        node?.let { return it }
        return try {
            initLogging()
            setCameraCapability(false)
            val n = Node(
                NodeConfig(
                    dataDir = ctx.filesDir.resolve("core").absolutePath,
                    deviceName = deviceName(),
                    platform = Platform.ANDROID,
                    role = Role.CLIENT,
                    port = 0u,
                    downloadDir = null,
                ),
                this,
            )
            node = n
            _ui.update { it.copy(peers = n.peers(), transfers = n.transfers().associateBy(::tkey), startError = null) }
            n
        } catch (e: Exception) {
            Log.e(TAG, "çekirdek başlatılamadı", e)
            _ui.update { it.copy(startError = e.message ?: e.toString()) }
            null
        }
    }

    fun deviceName(): String {
        val m = Build.MODEL ?: "Telefon"
        val b = Build.MANUFACTURER ?: ""
        return if (m.startsWith(b, ignoreCase = true)) m else "$b $m".trim()
    }

    fun tkey(t: TransferInfo) = "${t.direction}:${t.id}"

    fun refreshPeers() {
        node?.let { n -> _ui.update { it.copy(peers = n.peers()) } }
    }

    fun clearPairError() = _ui.update { it.copy(pairError = null) }

    // ---------- Host ----------

    override fun onEvent(event: Event) {
        when (event) {
            is Event.State -> {
                if (event.state == ConnState.CONNECTED) sentIcons.clear() else { Screen.onDisconnected(); main.post { app.kopru.ui.KeyPad.resetLocal() }; Suite.onDisconnected() }
                _ui.update { it.copy(conn = event.state, peer = event.peer, peers = node?.peers() ?: it.peers) }
                main.post { KopruService.onConnState(event.state) }
            }
            is Event.Paired -> _ui.update { it.copy(pairError = null, peers = node?.peers() ?: it.peers) }
            is Event.PairFailed -> _ui.update { it.copy(pairError = event.reason) }
            is Event.Discovered -> _ui.update { it.copy(discovered = event.servers) }
            is Event.Scanning -> {
                _ui.update { it.copy(scanning = event.on) }
                if (!event.on) main.post { KopruService.holdMulticast(false) }
            }
            is Event.ClipboardReceived -> main.post { setClipboard(event.text) }
            is Event.Transfer -> {
                val t = event.info
                _ui.update { it.copy(transfers = it.transfers + (tkey(t) to t)) }
                if (t.direction == uniffi.kopru_core.Direction.OUTGOING && (t.state == TransferState.FAILED || t.state == TransferState.CANCELLED)) ViewerLink.onOutgoingTransfer(t.name, true)
                main.post { KopruService.onTransfer(t) }
            }
            is Event.TransferRemoved -> _ui.update { s ->
                s.copy(transfers = s.transfers.filterValues { !(it.id == event.id && it.direction == event.direction) })
            }
            is Event.Phone -> main.post { handlePhone(event.cmd) }
            is Event.ScreenInfo -> Screen.onInfo(event.monitors, event.current.toInt(), event.active, event.error)
            is Event.ScreenNotice -> Screen.onSecure(event.secure)
            is Event.ScreenFrame -> Screen.onFrame(event.ptsUs.toLong(), event.recvUs.toLong(), event.key, event.data)
            is Event.Screen -> Unit
            is Event.Key -> Unit
            is Event.SuiteApps -> Suite.onApps(event.apps)
            is Event.SuiteEvent -> Suite.onEvent(event.app, event.name, event.json)
            is Event.SuiteReply -> Suite.onReply(event.req.toLong(), event.ok, event.json, event.error)
            is Event.SuiteCmd -> Unit
            is Event.PairCodeRotated -> Unit
            is Event.Cam, is Event.CamState, is Event.CamFrame -> Unit
            // Telefon tarafında beklenmeyen (PC'ye yönelik) olaylar.
            // PC bir bildirime yanıt yazdı: kaynak uygulamanın RemoteInput'una ilet (tek iş parçacıklı havuzda).
            is Event.NotificationReply -> replyExec.execute { NotifListener.reply(ctx, event.key, event.text) }
            is Event.Notification, is Event.NotificationRemoved, is Event.NotificationExt, is Event.NotificationReplyResult,
            is Event.AppIcon, is Event.Battery, is Event.NowPlaying, is Event.PointerMove, is Event.Pc -> Unit
        }
    }

    private fun handlePhone(cmd: PhoneCommand) {
        when (cmd) {
            is PhoneCommand.Ring -> if (cmd.on) Ringer.start(ctx) else Ringer.stop()
            is PhoneCommand.Media -> MediaControl.media(ctx, cmd.action)
            is PhoneCommand.Volume -> MediaControl.volume(ctx, cmd.action)
            is PhoneCommand.DismissNotification -> NotifListener.dismiss(cmd.key)
        }
    }

    /** Panoya PC'den gelen metni yaz. (Android 10+ arka planda yazmaya izin verir; okumaya vermez.) */
    private fun setClipboard(text: String) {
        val cm = ctx.getSystemService(ClipboardManager::class.java) ?: return
        lastClipSet = text
        cm.setPrimaryClip(ClipData.newPlainText("T-Bridge", text))
    }

    /** Uygulamanın kendi yazdığı son metin: ön plandaki dinleyici bunu geri göndermesin. */
    @Volatile
    var lastClipSet: String? = null

    override fun openIncoming(key: String, name: String, size: ULong): OpenedFd =
        try {
            IncomingFiles.open(key, name, size.toLong())
        } catch (e: Exception) {
            Log.w(TAG, "gelen dosya açılamadı", e)
            throw KopruException.Failed(e.message ?: ctx.getString(R.string.core_err_open_in))
        }

    override fun completeIncoming(key: String, ok: Boolean): String? = IncomingFiles.complete(key, ok)

    override fun openOutgoing(source: String): Int =
        try {
            OutgoingFiles.openFd(ctx, source)
        } catch (e: Exception) {
            Log.w(TAG, "giden dosya açılamadı", e)
            throw KopruException.Failed(e.message ?: ctx.getString(R.string.core_err_read_out))
        }

    // ---------- Kısayollar ----------

    fun sendClipboardText(text: String?): Boolean {
        val n = node ?: return false
        if (text.isNullOrEmpty()) return false
        return n.sendClipboard(text)
    }

    fun sendFiles(files: List<FileSource>): Int {
        val n = node ?: start() ?: return 0
        if (files.isEmpty()) return 0
        return try {
            n.sendFiles(files).size
        } catch (e: KopruException) {
            _ui.update { it.copy(pairError = e.message) }
            0
        }
    }

    fun activeTransfers(): Boolean =
        _ui.value.transfers.values.any { it.state == TransferState.ACTIVE }

    fun isConnected() = _ui.value.conn == ConnState.CONNECTED

    /**
     * Sınırlı bir mDNS tarama penceresi açar (bağlıyken hiçbir şey yapmaz). MulticastLock taramadan
     * ÖNCE alınır; pencere bitince çekirdeğin Scanning(false) olayıyla bırakılır.
     */
    fun rescan() {
        val n = node ?: return
        if (isConnected()) return n.nudge()
        KopruService.holdMulticast(true)
        n.rescan()
    }

    @Suppress("unused")
    private fun dirName(d: Direction) = d.name
}
