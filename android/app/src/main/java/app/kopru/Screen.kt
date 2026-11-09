package app.kopru

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.Surface
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import uniffi.kopru_core.MonitorInfo

/** Hazır kalite ayarları (PC `screen.rs` ile aynı değerler). */
enum class ScreenPreset(val label: String, val w: Int, val h: Int, val fps: Int, val kbps: Int) {
    ECO("Eko", 1280, 720, 15, 2000),
    BALANCED("Dengeli", 1920, 1080, 30, 5000),
    SMOOTH("Akıcı", 1920, 1080, 60, 8000),
}

data class ScreenUi(
    val monitors: List<MonitorInfo> = emptyList(),
    val current: Int = 0,
    /** PC paylaşımı başlattı (ilk kare gelmeden de true olabilir). */
    val active: Boolean = false,
    /** Telefon "ekranı göster" modunda mı (kullanıcı isteği). */
    val viewing: Boolean = false,
    val error: String? = null,
    /** PC güvenli masaüstünde (UAC / kilit ekranı): görüntü geçici olarak durdu. */
    val secure: Boolean = false,
    val videoW: Int = 0,
    val videoH: Int = 0,
    val fps: Int = 0,
    val latencyMs: Int = 0,
    val preset: ScreenPreset = ScreenPreset.BALANCED,
)

/**
 * Uzaktan ekran görüntüleme (telefon): PC'den gelen H.264 (Annex-B) karelerini MediaCodec ile SurfaceView'a çizer.
 * Düşük gecikme: KEY_LOW_LATENCY (API 30+), gerçek zamanlı öncelik, çıkış karesi hemen çizilir, geç/dolu kareler atılır.
 */
object Screen {
    private const val TAG = "kopru-screen"
    private val _ui = MutableStateFlow(ScreenUi())
    val ui: StateFlow<ScreenUi> = _ui

    private val lock = Any()
    private var surface: Surface? = null
    private var codec: MediaCodec? = null
    private var drain: Thread? = null

    @Volatile private var needKey = true
    private var lastKeyReq = 0L

    // fps / gecikme ölçümü
    private var frames = 0
    private var winStart = 0L
    private var baseOffset = Long.MAX_VALUE // min(varış - pts): sabit saat farkı

    // ---------------------------------------------------------------- denetim

    fun supported(): Boolean = Core.node?.peerSupportsScreen() == true

    fun startViewing(monitor: Int = _ui.value.current, preset: ScreenPreset = _ui.value.preset) {
        val n = Core.node ?: return
        if (!n.peerSupportsScreen()) {
            _ui.update { it.copy(error = "Bilgisayardaki T-Bridge ekran paylaşımını desteklemiyor") }
            return
        }
        baseOffset = Long.MAX_VALUE
        _ui.update { it.copy(viewing = true, error = null, secure = false, current = monitor, preset = preset, fps = 0, latencyMs = 0) }
        needKey = true
        n.sendScreenStart(monitor.toUInt(), preset.w.toUInt(), preset.h.toUInt(), preset.fps.toUInt(), preset.kbps.toUInt())
    }

    fun stopViewing() {
        _ui.update { it.copy(viewing = false, active = false, secure = false, fps = 0, latencyMs = 0) }
        Core.node?.sendScreenStop()
        release()
    }

    /** Uygulama arka plana gitti: pil için çözmeyi durdur (PC'ye de bildir). Dönünce `resume` ile yeniden başlar. */
    fun pause() {
        if (_ui.value.viewing) {
            Core.node?.sendScreenStop()
            release()
        }
    }

    fun resume() {
        if (_ui.value.viewing) startViewing()
    }

    fun setPreset(p: ScreenPreset) {
        _ui.update { it.copy(preset = p) }
        if (_ui.value.viewing) Core.node?.sendScreenConfig(p.w.toUInt(), p.h.toUInt(), p.fps.toUInt(), p.kbps.toUInt())
        needKey = true
    }

    fun setMonitor(index: Int) {
        if (_ui.value.viewing) startViewing(monitor = index) else _ui.update { it.copy(current = index) }
    }

    // ---------------------------------------------------------------- olaylar (Rust iş parçacığı)

    fun onInfo(monitors: List<MonitorInfo>, current: Int, active: Boolean, error: String?) {
        // Paylaşım bittiyse güvenli masaüstü notu da kalkar; aktifken (yeniden kurulum bildirimi) not korunur.
        _ui.update { it.copy(monitors = monitors, current = if (it.viewing) it.current else current, active = active, error = error, secure = it.secure && active) }
    }

    /** PC güvenli masaüstüne geçti / döndü (eski PC bu bildirimi göndermez). */
    fun onSecure(secure: Boolean) {
        _ui.update { it.copy(secure = secure) }
        if (!secure) needKey = true // devam eden ilk karede anahtar kare iste
    }

    fun onDisconnected() {
        _ui.update { it.copy(viewing = false, active = false, secure = false, fps = 0, latencyMs = 0) }
        release()
    }

    fun onFrame(ptsUs: Long, key: Boolean, data: ByteArray) {
        val now = SystemClock.elapsedRealtime()
        // Gecikme ölçümü: varış zamanı - pts, en küçük değere göre göreli.
        val off = now - ptsUs / 1000
        if (off < baseOffset) baseOffset = off
        frames++
        if (winStart == 0L) winStart = now
        if (now - winStart >= 1000) {
            val fps = (frames * 1000L / (now - winStart)).toInt()
            val lat = (off - baseOffset).toInt().coerceIn(0, 9999)
            _ui.update { it.copy(fps = fps, latencyMs = lat) }
            frames = 0
            winStart = now
        }
        synchronized(lock) {
            val c = codec ?: return
            if (needKey && !key) {
                requestKey(now)
                return
            }
            try {
                // Kısa bekleme (~10 ms): anlık doluluk anahtar kare fırtınası yaratmasın; yalnız kalıcı dolulukta at.
                val i = c.dequeueInputBuffer(10_000)
                if (i < 0) {
                    // Çözücü kalıcı dolu: kareyi at, sonraki anahtar kareye kadar P'leri de at (görüntü bozulmasın).
                    needKey = true
                    requestKey(now)
                    return
                }
                val buf = c.getInputBuffer(i) ?: return
                if (data.size > buf.capacity()) {
                    c.queueInputBuffer(i, 0, 0, ptsUs, 0)
                    needKey = true
                    requestKey(now)
                    return
                }
                buf.clear()
                buf.put(data)
                c.queueInputBuffer(i, 0, data.size, ptsUs, if (key) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0)
                if (key) needKey = false
            } catch (e: Exception) {
                Log.w(TAG, "çözücü hatası: $e")
                resetCodecLocked()
                needKey = true
                requestKey(now)
            }
        }
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
        frames = 0
        winStart = 0
        baseOffset = Long.MAX_VALUE
    }

    private fun resetCodecLocked() {
        stopCodecLocked()
        startCodecLocked()
    }

    private fun startCodecLocked() {
        val s = surface ?: return
        if (codec != null) return
        try {
            val fmt = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, 1920, 1080).apply {
                // Uyarlanabilir oynatma: çözünürlük değişimi (kalite/monitör) çözücüyü yeniden kurmadan işlenir.
                setInteger(MediaFormat.KEY_MAX_WIDTH, 3840)
                setInteger(MediaFormat.KEY_MAX_HEIGHT, 2160)
                if (Build.VERSION.SDK_INT >= 30) setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
                setInteger(MediaFormat.KEY_PRIORITY, 0) // gerçek zamanlı
            }
            val c = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            c.configure(fmt, s, null, 0)
            c.start()
            codec = c
            val t = Thread({ drainLoop(c) }, "screen-drain")
            drain = t
            t.start()
        } catch (e: Exception) {
            Log.w(TAG, "çözücü başlatılamadı: $e")
            codec = null
            _ui.update { it.copy(error = "Video çözücü başlatılamadı") }
        }
    }

    private fun stopCodecLocked() {
        val c = codec ?: return
        codec = null
        try { c.stop() } catch (_: Exception) {}
        try { c.release() } catch (_: Exception) {}
        drain = null
    }

    /** Çıkış karelerini hemen yüzeye çizer (render=true -> zaman damgası beklenmez). */
    private fun drainLoop(c: MediaCodec) {
        val info = MediaCodec.BufferInfo()
        while (true) {
            if (codec !== c) return
            try {
                when (val i = c.dequeueOutputBuffer(info, 10_000)) {
                    MediaCodec.INFO_TRY_AGAIN_LATER -> {}
                    MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        val f = c.outputFormat
                        val w = f.getInteger(MediaFormat.KEY_WIDTH)
                        val h = f.getInteger(MediaFormat.KEY_HEIGHT)
                        _ui.update { it.copy(videoW = w, videoH = h) }
                    }
                    else -> if (i >= 0) c.releaseOutputBuffer(i, true)
                }
            } catch (_: Exception) {
                return
            }
        }
    }
}
