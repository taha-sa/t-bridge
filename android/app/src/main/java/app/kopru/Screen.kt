package app.kopru

import androidx.annotation.StringRes
import android.content.res.Resources
import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.os.SystemClock
import app.kopru.AppLog as Log
import android.view.Surface
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import uniffi.kopru_core.MonitorInfo
import uniffi.kopru_core.ScreenStatsReport
import uniffi.kopru_core.screenNowUs
import kotlin.math.max
import kotlin.math.min

/**
 * Hazır kalite ayarları. Çözünürlük telefonun GERÇEK görüntü alanından (piksel) gelir; `cap*` yalnızca üst sınırdır
 * (Eko 720p, diğerleri 1080p; yatay/dikey ayrımı kutu yönüne göre). Bit hızını PC otomatik seçer (yeni PC); eski PC için `legacyKbps`.
 */
enum class ScreenPreset(@StringRes val label: Int, val capW: Int, val capH: Int, val fps: Int, val legacyKbps: Int) {
    ECO(R.string.scr_preset_eco, 1280, 720, 15, 2000),
    BALANCED(R.string.scr_preset_balanced, 1920, 1080, 30, 5000),
    SMOOTH(R.string.scr_preset_smooth, 1920, 1080, 60, 8000),
}

data class ScreenUi(
    val monitors: List<MonitorInfo> = emptyList(),
    val current: Int = 0,
    /** PC paylaşımı başlattı (ilk kare gelmeden de true olabilir). */
    val active: Boolean = false,
    /** Telefon "ekranı göster" modunda mı (kullanıcı isteği). */
    val viewing: Boolean = false,
    val error: String? = null,
    /** Telefonun kendi hatası (kaynak kimliği); 0 = yok. Arayüz yerelleştirilmiş metni çözer. */
    @StringRes val errorRes: Int = 0,
    /** PC güvenli masaüstünde (UAC / kilit ekranı): görüntü geçici olarak durdu. */
    val secure: Boolean = false,
    val videoW: Int = 0,
    val videoH: Int = 0,
    /** Ekrana çizilen kare/sn. */
    val fps: Int = 0,
    /** Uçtan uca gecikme (PC yakalama → telefon çizim), ms. Saat senkronu yoksa göreli kestirim. */
    val latencyMs: Int = 0,
    val preset: ScreenPreset = ScreenPreset.BALANCED,
)

/** Gizli teşhis katmanının satırları (fps göstergesine uzun basınca açılır). */
data class ScreenDiag(val lines: List<String> = emptyList())

/**
 * Uzaktan ekran görüntüleme (telefon): PC'den gelen H.264 (Annex-B) karelerini MediaCodec ile SurfaceView'a çizer.
 *
 * Boru hattı: Rust olayı → [onFrame] (hızlı, kuyruğa koyar) → besleyici iş parçacığı → MediaCodec → çıkış iş parçacığı
 * (render=true, anında çizim). Düşük gecikme: KEY_LOW_LATENCY, KEY_PRIORITY=0, yüksek KEY_OPERATING_RATE; kuyruk dolarsa
 * bayat kareler atılıp en yeni anahtar kareden devam edilir (IDR istenir).
 *
 * Görüntü alanı: görüntüleyici ölçülünce (veya döndürme/yakınlaştırma sonrası) piksel boyutu PC'ye bildirilir; PC kodlama
 * boyutunu buna göre seçer (1:1, yeniden örnekleme yok).
 */
object Screen {
    private const val TAG = "kopru-screen"
    private const val MAX_Q = 6
    private val _ui = MutableStateFlow(ScreenUi())
    val ui: StateFlow<ScreenUi> = _ui
    private val _diag = MutableStateFlow(ScreenDiag())
    val diag: StateFlow<ScreenDiag> = _diag
    private val main = Handler(Looper.getMainLooper())

    private val lock = Any()
    private var surface: Surface? = null
    @Volatile private var codec: MediaCodec? = null
    private var feedThread: Thread? = null
    private var drainThread: Thread? = null

    @Volatile private var needKey = true
    @Volatile private var lastKeyReq = 0L

    // ---------------------------------------------------------------- görüntü alanı
    private var viewW = 0
    private var viewH = 0
    private var zoom = 1f
    private var sentW = 0
    private var sentH = 0
    @Volatile private var pendingStart = false
    private var startAfterViewport: Runnable? = null
    private var configJob: Runnable? = null

    // ---------------------------------------------------------------- kare kuyruğu
    private class Frame(val pts: Long, val recvUs: Long, val key: Boolean, val data: ByteArray)
    private val qLock = ReentrantLock()
    private val qCond = qLock.newCondition()
    private val queue = ArrayDeque<Frame>()

    // çözücüye girişte pts → (giriş zamanı, ağdan alınış) halkası
    private val ringPts = LongArray(64)
    private val ringIn = LongArray(64)
    private val ringRecv = LongArray(64)
    private var ringN = 0

    // ---------------------------------------------------------------- istatistik (1 sn pencere)
    private val st = Any()
    private var winStart = 0L
    private var rx = 0
    private var rendered = 0
    private var decSumNs = 0L
    private var decN = 0
    private var r2rSumUs = 0L
    private var e2eSumMs = 0L
    private var e2eN = 0
    private var e2eMax = 0L
    private var late = 0
    private var inputWaits = 0
    private var qMax = 0
    private var baseRelUs = Long.MAX_VALUE // saat senkronu yokken göreli gecikme tabanı
    @Volatile private var clockValid = false
    @Volatile private var clockOffsetUs = 0L
    @Volatile private var clockRttUs = 0L

    // ---------------------------------------------------------------- denetim

    fun supported(): Boolean = Core.node?.peerSupportsScreen() == true

    private fun tune(): Boolean = Core.node?.peerSupportsScreenTune() == true

    private fun displaySize(): Pair<Int, Int> {
        val m = Resources.getSystem().displayMetrics
        return m.widthPixels to m.heightPixels
    }

    /** Görüntüleyicinin ölçülen piksel boyutu (kutu). Yeniden ölçüm/döndürme burayı çağırır. */
    fun setViewport(w: Int, h: Int) {
        if (w < 64 || h < 64) return
        main.post {
            val changed = w != viewW || h != viewH
            viewW = w
            viewH = h
            if (pendingStart) {
                doStart()
            } else if (changed && _ui.value.viewing) {
                scheduleConfig(250)
            }
        }
    }

    /** Çimdik yakınlaştırma düzeyi (kutuya göre). Yüksek yakınlaştırmada PC daha yüksek çözünürlük gönderir (metin keskin kalır). */
    fun setZoom(z: Float) {
        main.post {
            val q = zoomQuant(z)
            if (q != zoom) {
                zoom = q
                if (_ui.value.viewing && !pendingStart) scheduleConfig(450)
            }
        }
    }

    private fun scheduleConfig(delayMs: Long) {
        configJob?.let { main.removeCallbacks(it) }
        val r = Runnable { sendConfigIfChanged() }
        configJob = r
        main.postDelayed(r, delayMs)
    }

    /** PC'ye bildirilecek istek boyutu: kutu × yakınlaştırma, ön ayar sınırına göre (eksen başına) kırpılmış, çift sayı. */
    private fun requestSize(): Pair<Int, Int> = requestSize(viewW, viewH, zoom, _ui.value.preset)

    private fun sendConfigIfChanged() {
        val v = _ui.value
        if (!v.viewing) return
        val (w, h) = requestSize()
        if (!sizeChangedEnough(sentW, sentH, w, h)) return
        sentW = w
        sentH = h
        val p = v.preset
        Core.node?.sendScreenConfig(w.toUInt(), h.toUInt(), p.fps.toUInt(), kbpsFor(p).toUInt())
        needKey = true
        Log.i(TAG, "görüntü alanı $w x $h (kutu $viewW x $viewH, yakınlaştırma $zoom)")
    }

    private fun kbpsFor(p: ScreenPreset) = if (tune()) 0 else p.legacyKbps

    fun startViewing(monitor: Int = _ui.value.current, preset: ScreenPreset = _ui.value.preset) {
        val n = Core.node ?: return
        if (!n.peerSupportsScreen()) {
            _ui.update { it.copy(error = null, errorRes = R.string.scr_err_unsupported) }
            return
        }
        _ui.update { it.copy(viewing = true, error = null, errorRes = 0, secure = false, current = monitor, preset = preset, fps = 0, latencyMs = 0) }
        needKey = true
        resetStats()
        if (viewW > 0) {
            doStart()
        } else {
            // Görüntüleyici henüz ölçülmedi: ölçüm gelince başlat (çifte müzakere olmasın); gelmezse ekran boyutuyla başla.
            pendingStart = true
            val r = Runnable { if (pendingStart) { val (dw, dh) = displaySize(); viewW = dw; viewH = dh; doStart() } }
            startAfterViewport = r
            main.postDelayed(r, 400)
        }
    }

    private fun doStart() {
        pendingStart = false
        startAfterViewport?.let { main.removeCallbacks(it) }
        val n = Core.node ?: return
        val v = _ui.value
        if (!v.viewing) return
        val (w, h) = requestSize()
        sentW = w
        sentH = h
        n.screenSync(true)
        n.sendScreenStart(v.current.toUInt(), w.toUInt(), h.toUInt(), v.preset.fps.toUInt(), kbpsFor(v.preset).toUInt())
        Log.i(TAG, "başlat: istek $w x $h @${v.preset.fps} (kutu $viewW x $viewH)")
    }

    fun stopViewing() {
        pendingStart = false
        _ui.update { it.copy(viewing = false, active = false, secure = false, fps = 0, latencyMs = 0) }
        Core.node?.sendScreenStop()
        Core.node?.screenSync(false)
        release()
    }

    /** Uygulama arka plana gitti: pil için çözmeyi durdur (PC'ye de bildir). Dönünce `resume` ile yeniden başlar. */
    fun pause() {
        if (_ui.value.viewing) {
            Core.node?.sendScreenStop()
            Core.node?.screenSync(false)
            release()
        }
    }

    fun resume() {
        if (_ui.value.viewing) startViewing()
    }

    fun setPreset(p: ScreenPreset) {
        _ui.update { it.copy(preset = p) }
        if (_ui.value.viewing) {
            val (w, h) = requestSize()
            sentW = w
            sentH = h
            Core.node?.sendScreenConfig(w.toUInt(), h.toUInt(), p.fps.toUInt(), kbpsFor(p).toUInt())
        }
        needKey = true
    }

    fun setMonitor(index: Int) {
        if (_ui.value.viewing) startViewing(monitor = index) else _ui.update { it.copy(current = index) }
    }

    // ---------------------------------------------------------------- olaylar (Rust iş parçacığı)

    fun onInfo(monitors: List<MonitorInfo>, current: Int, active: Boolean, error: String?) {
        // Paylaşım bittiyse güvenli masaüstü notu da kalkar; aktifken (yeniden kurulum bildirimi) not korunur.
        _ui.update { it.copy(monitors = monitors, current = if (it.viewing) it.current else current, active = active, error = error, errorRes = 0, secure = it.secure && active) }
    }

    /** PC güvenli masaüstüne geçti / döndü (eski PC bu bildirimi göndermez). */
    fun onSecure(secure: Boolean) {
        _ui.update { it.copy(secure = secure) }
        if (!secure) needKey = true // devam eden ilk karede anahtar kare iste
    }

    fun onDisconnected() {
        pendingStart = false
        _ui.update { it.copy(viewing = false, active = false, secure = false, fps = 0, latencyMs = 0) }
        release()
    }

    /** Rust olay iş parçacığı: HIZLI dönmeli (QUIC okumasını bekletmesin). Kareyi kuyruğa koyar. */
    fun onFrame(ptsUs: Long, recvUs: Long, key: Boolean, data: ByteArray) {
        val now = SystemClock.elapsedRealtime()
        var dropped = 0
        var queuedSize = -1
        qLock.withLock {
            if (codec == null) {
                synchronized(st) { rx++ }
                return
            }
            if (needKey && !key) {
                synchronized(st) { rx++ }
                requestKey(now)
                return
            }
            if (queue.size >= MAX_Q) {
                // Çözücü yetişemiyor: bayat kareleri at; kuyrukta anahtar kare varsa ondan devam, yoksa yenisini bekle.
                dropped = dropStaleLocked()
                if (!key && queue.isEmpty()) {
                    synchronized(st) { rx++; late += dropped }
                    needKey = true
                    requestKey(now)
                    return
                }
            }
            queue.addLast(Frame(ptsUs, recvUs, key, data))
            queuedSize = queue.size
            qCond.signalAll()
        }
        synchronized(st) {
            rx++
            if (dropped > 0) late += dropped
            if (queuedSize > qMax) qMax = queuedSize
        }
    }

    /** Kuyruktaki en yeni anahtar kareye kadar olanları at (yoksa hepsini). Atılan sayı döner. */
    private fun dropStaleLocked(): Int {
        var lastKey = -1
        for ((i, f) in queue.withIndex()) if (f.key) lastKey = i
        val n = if (lastKey >= 0) lastKey else queue.size
        repeat(n) { queue.removeFirst() }
        return n
    }

    private fun requestKey(now: Long) {
        if (now - lastKeyReq > 300) {
            lastKeyReq = now
            Core.node?.sendScreenKeyframe()
        }
    }

    // ---------------------------------------------------------------- yüzey / çözücü

    fun attach(s: Surface) {
        synchronized(lock) {
            surface = s
            startCodecLocked()
        }
        needKey = true
        if (_ui.value.viewing) Core.node?.sendScreenKeyframe()
    }

    fun detach() {
        synchronized(lock) {
            stopCodecLocked()
            surface = null
        }
    }

    private fun release() {
        synchronized(lock) { stopCodecLocked() }
        resetStats()
    }

    private fun resetStats() {
        synchronized(st) {
            winStart = 0
            rx = 0; rendered = 0; decSumNs = 0; decN = 0; r2rSumUs = 0
            e2eSumMs = 0; e2eN = 0; e2eMax = 0; late = 0; inputWaits = 0; qMax = 0
            baseRelUs = Long.MAX_VALUE
        }
        _diag.value = ScreenDiag()
    }

    private fun resetCodecLocked() {
        stopCodecLocked()
        startCodecLocked()
    }

    private fun startCodecLocked() {
        val s = surface ?: return
        if (codec != null) return
        val v = _ui.value
        val w0 = if (v.videoW > 0) v.videoW else 1920
        val h0 = if (v.videoH > 0) v.videoH else 1080
        // Önce tam gerçek zamanlı ayarlarla, olmazsa yalın biçimle yapılandır (bazı çözücüler anahtarları reddeder).
        for (full in listOf(true, false)) {
            try {
                val fmt = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w0, h0).apply {
                    // Uyarlanabilir oynatma: çözünürlük/yön değişimi (kalite, döndürme, monitör) çözücüyü yeniden kurmadan işlenir.
                    setInteger(MediaFormat.KEY_MAX_WIDTH, 3840)
                    setInteger(MediaFormat.KEY_MAX_HEIGHT, 2160)
                    if (full) {
                        if (Build.VERSION.SDK_INT >= 30) setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
                        setInteger(MediaFormat.KEY_PRIORITY, 0) // gerçek zamanlı
                        setInteger(MediaFormat.KEY_OPERATING_RATE, 240) // çözücüyü en yüksek hıza kilitle (>60 fps)
                    }
                }
                val c = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
                try {
                    c.configure(fmt, s, null, 0)
                    c.start()
                } catch (e: Exception) {
                    try { c.release() } catch (_: Exception) {}
                    throw e
                }
                codec = c
                feedThread = Thread({ feedLoop(c) }, "screen-feed").apply { start() }
                drainThread = Thread({ drainLoop(c) }, "screen-drain").apply { start() }
                Log.i(TAG, "çözücü hazır: ${c.name} (gerçek zamanlı ayarlar=$full)")
                return
            } catch (e: Exception) {
                Log.w(TAG, "çözücü başlatılamadı (gerçek zamanlı=$full): $e")
            }
        }
        codec = null
        _ui.update { it.copy(error = null, errorRes = R.string.scr_err_decoder) }
    }

    private fun stopCodecLocked() {
        val c = codec ?: return
        codec = null
        val ft = feedThread
        val dt = drainThread
        feedThread = null
        drainThread = null
        qLock.withLock { queue.clear(); qCond.signalAll() }
        val cur = Thread.currentThread()
        if (ft != null && ft !== cur) runCatching { ft.join(300) }
        if (dt != null && dt !== cur) runCatching { dt.join(300) }
        try { c.stop() } catch (_: Exception) {}
        try { c.release() } catch (_: Exception) {}
    }

    /** Çözücü hatası: sıfırla ve anahtar kare iste (yüzey varsa yeniden kurar). */
    private fun codecFailed(c: MediaCodec, why: Exception) {
        Log.w(TAG, "çözücü hatası: $why")
        synchronized(lock) {
            if (codec !== c) return
            resetCodecLocked()
        }
        needKey = true
        requestKey(SystemClock.elapsedRealtime())
    }

    /** Kuyruktan çözücü girişine besler. Giriş tamponu gelmezse kareyi atar (gecikme birikmesin) ve IDR ister. */
    private fun feedLoop(c: MediaCodec) {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY)
        while (codec === c) {
            val f: Frame = qLock.withLock {
                while (queue.isEmpty()) {
                    if (codec !== c) return
                    qCond.await(100, TimeUnit.MILLISECONDS)
                }
                // Kuyrukta daha yeni bir anahtar kare varsa öncekiler bayattır.
                var lastKey = -1
                for ((i, x) in queue.withIndex()) if (x.key) lastKey = i
                if (lastKey > 0) {
                    repeat(lastKey) { queue.removeFirst() }
                    synchronized(st) { late += lastKey }
                }
                queue.removeFirst()
            }
            try {
                feed(c, f)
            } catch (e: Exception) {
                codecFailed(c, e)
                return
            }
        }
    }

    private fun feed(c: MediaCodec, f: Frame) {
        val now = SystemClock.elapsedRealtime()
        if (needKey && !f.key) {
            requestKey(now)
            return
        }
        var i = -1
        for (attempt in 0 until 3) {
            i = c.dequeueInputBuffer(4_000)
            if (i >= 0) break
            synchronized(st) { inputWaits++ }
        }
        if (i < 0) {
            // Çözücü kalıcı dolu: kareyi at, sonraki anahtar kareye kadar P'leri de at (görüntü bozulmasın).
            synchronized(st) { late++ }
            needKey = true
            requestKey(now)
            return
        }
        val buf = c.getInputBuffer(i)
        if (buf == null || f.data.size > buf.capacity()) {
            c.queueInputBuffer(i, 0, 0, f.pts, 0)
            needKey = true
            requestKey(now)
            return
        }
        buf.clear()
        buf.put(f.data)
        synchronized(ringPts) {
            val k = ringN++ and 63
            ringPts[k] = f.pts
            ringIn[k] = System.nanoTime()
            ringRecv[k] = f.recvUs
        }
        c.queueInputBuffer(i, 0, f.data.size, f.pts, if (f.key) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0)
        if (f.key) needKey = false
    }

    /** Çıkış karelerini hemen yüzeye çizer (render=true -> zaman damgası beklenmez) ve ölçer. */
    private fun drainLoop(c: MediaCodec) {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY)
        val info = MediaCodec.BufferInfo()
        while (codec === c) {
            try {
                when (val i = c.dequeueOutputBuffer(info, 10_000)) {
                    MediaCodec.INFO_TRY_AGAIN_LATER -> {}
                    MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        val f = c.outputFormat
                        val w = f.getInteger(MediaFormat.KEY_WIDTH)
                        val h = f.getInteger(MediaFormat.KEY_HEIGHT)
                        _ui.update { it.copy(videoW = w, videoH = h) }
                    }
                    else -> if (i >= 0) {
                        val pts = info.presentationTimeUs
                        c.releaseOutputBuffer(i, true)
                        onRendered(pts)
                    }
                }
            } catch (e: Exception) {
                if (codec === c) codecFailed(c, e)
                return
            }
            flushIfDue()
        }
    }

    private fun onRendered(pts: Long) {
        val nowNs = System.nanoTime()
        var inNs = 0L
        var recvUs = 0L
        synchronized(ringPts) {
            for (k in 0 until 64) {
                if (ringPts[k] == pts && ringIn[k] != 0L) {
                    inNs = ringIn[k]
                    recvUs = ringRecv[k]
                    ringIn[k] = 0
                    break
                }
            }
        }
        val nowUs = screenNowUs().toLong()
        val r2r = if (recvUs > 0) nowUs - recvUs else 0L
        val e2e: Long = if (clockValid) {
            (nowUs + clockOffsetUs - pts) / 1000
        } else if (recvUs > 0) {
            // Saat senkronu yok (eski PC): en küçük varış−pts farkına göre göreli gecikme.
            val rel = recvUs - pts
            if (rel < baseRelUs) baseRelUs = rel
            (rel - baseRelUs + r2r) / 1000
        } else 0L
        synchronized(st) {
            rendered++
            if (inNs != 0L) { decSumNs += nowNs - inNs; decN++ }
            r2rSumUs += r2r
            val e = e2e.coerceIn(0, 9999)
            e2eSumMs += e
            e2eN++
            if (e > e2eMax) e2eMax = e
        }
    }

    /** 1 sn dolduysa pencereyi özetle: arayüz, teşhis katmanı, günlük ve PC'ye rapor. */
    private fun flushIfDue() {
        val now = SystemClock.elapsedRealtime()
        val s: StatsSnapshot
        synchronized(st) {
            if (winStart == 0L) { winStart = now; return }
            val dt = now - winStart
            if (dt < 1000) return
            val sec = dt / 1000.0
            s = StatsSnapshot(
                rxFps = rx / sec, renderFps = rendered / sec,
                decodeMs = if (decN > 0) decSumNs / decN / 1e6 else 0.0,
                r2rMs = if (rendered > 0) r2rSumUs / rendered / 1000.0 else 0.0,
                e2eMs = if (e2eN > 0) e2eSumMs.toDouble() / e2eN else 0.0,
                e2eMax = e2eMax, late = late, inputWaits = inputWaits, qMax = qMax,
            )
            winStart = now; rx = 0; rendered = 0; decSumNs = 0; decN = 0; r2rSumUs = 0
            e2eSumMs = 0; e2eN = 0; e2eMax = 0; late = 0; inputWaits = 0; qMax = 0
        }
        val n = Core.node
        if (n != null) {
            try {
                val c = n.screenClock()
                clockValid = c.valid
                clockOffsetUs = c.offsetUs
                clockRttUs = c.rttUs.toLong()
            } catch (_: Exception) {}
        }
        val rttMs = clockRttUs / 1000.0
        _ui.update { it.copy(fps = Math.round(s.renderFps).toInt(), latencyMs = Math.round(s.e2eMs).toInt()) }
        val v = _ui.value
        _diag.value = ScreenDiag(
            listOf(
                "uçtan uca ${fmt1(s.e2eMs)} ms (en çok ${s.e2eMax})" + if (clockValid) "" else " · saat senkronu yok (göreli)",
                "alınan ${fmt1(s.rxFps)}/s · çizilen ${fmt1(s.renderFps)}/s",
                "çözme ${fmt1(s.decodeMs)} ms · alındı→çizildi ${fmt1(s.r2rMs)} ms",
                "geç atılan ${s.late} · giriş beklemesi ${s.inputWaits} · kuyruk ${s.qMax}",
                "RTT ${fmt1(rttMs)} ms · ${if (tune()) "ack+datagram" else "eski PC (ack yok)"}",
                "video ${v.videoW}x${v.videoH} · ${v.preset.name} · istek ${sentW}x${sentH} (kutu ${viewW}x${viewH}, ×${zoom})",
            ),
        )
        Log.i(
            TAG,
            "[ekran-tel] alınan ${fmt1(s.rxFps)}/s çizilen ${fmt1(s.renderFps)}/s | çözme ${fmt1(s.decodeMs)} ms alındı→çizildi ${fmt1(s.r2rMs)} ms | uçtan uca ${fmt1(s.e2eMs)}/${s.e2eMax} ms | geç ${s.late} bekleme ${s.inputWaits} kuyruk ${s.qMax} | RTT ${fmt1(rttMs)} ms",
        )
        if (v.viewing && n != null) {
            n.sendScreenStats(
                ScreenStatsReport(
                    rxFps10 = (s.rxFps * 10).toInt().coerceIn(0, 65535).toUShort(),
                    renderFps10 = (s.renderFps * 10).toInt().coerceIn(0, 65535).toUShort(),
                    decodeMs10 = (s.decodeMs * 10).toInt().coerceIn(0, 65535).toUShort(),
                    recvToRenderMs10 = (s.r2rMs * 10).toInt().coerceIn(0, 65535).toUShort(),
                    e2eMs = s.e2eMs.toInt().coerceIn(0, 65535).toUShort(),
                    e2eMaxMs = s.e2eMax.toInt().coerceIn(0, 65535).toUShort(),
                    lateDrops = s.late.coerceIn(0, 65535).toUShort(),
                    inputWaits = s.inputWaits.coerceIn(0, 65535).toUShort(),
                    queueMax = s.qMax.coerceIn(0, 65535).toUShort(),
                    rttMs10 = (rttMs * 10).toInt().coerceIn(0, 65535).toUShort(),
                ),
            )
        }
    }

    private class StatsSnapshot(
        val rxFps: Double, val renderFps: Double, val decodeMs: Double, val r2rMs: Double,
        val e2eMs: Double, val e2eMax: Long, val late: Int, val inputWaits: Int, val qMax: Int,
    )

    private fun fmt1(v: Double) = String.format(java.util.Locale.US, "%.1f", v)

    // ---------------------------------------------------------------- saf yardımcılar (birim testli)

    /** Yakınlaştırma düzeyini 1 / 1.5 / 2'ye yuvarlar (sık müzakereyi önler). */
    fun zoomQuant(z: Float): Float = when {
        z < 1.25f -> 1f
        z < 1.75f -> 1.5f
        else -> 2f
    }

    /** Kutu × yakınlaştırma, ön ayar üst sınırına (eksen başına, yöne göre) kırpılmış, çift sayı. */
    fun requestSize(boxW: Int, boxH: Int, zoom: Float, p: ScreenPreset): Pair<Int, Int> {
        val bw = max(boxW, 2)
        val bh = max(boxH, 2)
        val landscape = bw >= bh
        val z = zoom.coerceIn(1f, 2f)
        val capW = ((if (landscape) p.capW else p.capH) * z).toInt()
        val capH = ((if (landscape) p.capH else p.capW) * z).toInt()
        val w = min((bw * z).toInt(), capW)
        val h = min((bh * z).toInt(), capH)
        return (max(w, 2) and 1.inv()) to (max(h, 2) and 1.inv())
    }

    /** Yeniden müzakere gerekli mi: yön değişti ya da alan >%6 değişti. */
    fun sizeChangedEnough(oldW: Int, oldH: Int, w: Int, h: Int): Boolean {
        if (oldW <= 0 || oldH <= 0) return true
        if ((oldW >= oldH) != (w >= h)) return true
        val a = oldW.toLong() * oldH
        val b = w.toLong() * h
        return Math.abs(a - b) * 100 > a * 6
    }
}
