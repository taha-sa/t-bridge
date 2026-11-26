package app.kopru

import androidx.annotation.StringRes
import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import app.kopru.AppLog as Log
import android.util.Size
import android.view.OrientationEventListener
import android.view.Surface
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.Preview
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import uniffi.kopru_core.CamCmd

data class CamUi(
    val streaming: Boolean = false,
    val front: Boolean = false,
    /** İstenen dikey çözünürlük: 720 veya 1080. */
    val res: Int = 720,
    val mirror: Boolean = false,
    val torch: Boolean = false,
    val exposure: Int = 0,
    val exposureRange: IntRange = 0..0,
    val zoom: Float = 0f,
    /** Karenin dik görünmesi için PC'nin uygulayacağı saat yönü derecesi (0/90/180/270). */
    val rotation: Int = 0,
    val w: Int = 0,
    val h: Int = 0,
    @StringRes val error: Int? = null,
)

/**
 * Telefon kamerası -> PC (sanal web kamerası): CameraX Preview, çıkışı MediaCodec H.264 kodlayıcının giriş yüzeyine verir
 * (CBR, 2 sn GOP, düşük gecikme); Annex-B kareler çekirdek üzerinden PC'ye tek yönlü QUIC akışıyla gider.
 * Hizmet "camera" türünde ön plan hizmetidir (Android 14): ekran kapalıyken de yayın sürer.
 */
class CameraService : Service(), LifecycleOwner {
    private val registry = LifecycleRegistry(this)
    override val lifecycle: Lifecycle get() = registry

    private var provider: ProcessCameraProvider? = null
    private var camera: Camera? = null
    private var enc: Encoder? = null
    private var preview: Preview? = null
    private var orient: OrientationEventListener? = null
    private var generation = 0

    override fun onCreate() {
        super.onCreate()
        registry.currentState = Lifecycle.State.STARTED
        instance = this
    }

    override fun onBind(intent: Intent?): IBinder? = null

    @SuppressLint("InlinedApi")
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CH, getString(R.string.cam_channel_name), NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val n: Notification = NotificationCompat.Builder(this, CH)
            .setSmallIcon(R.drawable.ic_stat)
            .setContentTitle(getString(R.string.cam_notif_title))
            .setContentText(getString(R.string.cam_notif_text))
            .setOngoing(true)
            .setContentIntent(open)
            .build()
        try {
            if (Build.VERSION.SDK_INT >= 29) startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_CAMERA) else startForeground(ID, n)
        } catch (e: SecurityException) {
            Log.w(TAG, "kamera ön plan izni yok: $e")
            fail(R.string.cam_err_permission)
            return START_NOT_STICKY
        } catch (e: IllegalStateException) {
            Log.w(TAG, "kamera ön plan hizmeti başlatılamadı: $e")
            fail(R.string.cam_err_open)
            return START_NOT_STICKY
        }
        startCamera()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        stopCamera()
        registry.currentState = Lifecycle.State.DESTROYED
        instance = null
        super.onDestroy()
    }

    // ---------------------------------------------------------------- kamera

    fun restart() {
        stopCamera()
        startCamera()
    }

    private fun startCamera() {
        if (ContextCompat.checkSelfPermission(this, android.Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            fail(R.string.cam_err_permission)
            return
        }
        val session = ++generation
        val fut = ProcessCameraProvider.getInstance(this)
        fut.addListener({
            if (session != generation) return@addListener
            try {
                val p = fut.get()
                provider = p
                val ui = ui.value
                val rs = ResolutionSelector.Builder()
                    .setResolutionStrategy(
                        ResolutionStrategy(
                            if (ui.res >= 1080) Size(1920, 1080) else Size(1280, 720),
                            ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER,
                        ),
                    )
                    .build()
                val preview = Preview.Builder().setResolutionSelector(rs).build()
                this.preview = preview
                val exec = ContextCompat.getMainExecutor(this)
                preview.setSurfaceProvider(exec) { req ->
                    val size = req.resolution
                    val fps = 30
                    val kbps = if (size.height >= 1000) 6000 else 3000
                    if (session != generation) {
                        req.willNotProvideSurface()
                        return@setSurfaceProvider
                    }
                    val e = try {
                        Encoder(size.width, size.height, fps, kbps)
                    } catch (ex: Exception) {
                        Log.w(TAG, "kamera kodlayıcısı başlatılamadı: $ex")
                        req.willNotProvideSurface()
                        fail(R.string.cam_err_open)
                        return@setSurfaceProvider
                    }
                    enc = e
                    req.provideSurface(e.surface, exec) { e.release() }
                    // Cihaz yönüne göre dik görünmesi için gereken dönüş; PC döndürür (kodlayıcı boyutu değişmez).
                    req.setTransformationInfoListener(exec) { info ->
                        if (session != generation) return@setTransformationInfoListener
                        _ui.update { it.copy(rotation = info.rotationDegrees) }
                        publishState(size.width, size.height)
                    }
                    publishState(size.width, size.height)
                }
                val sel = if (ui.front) CameraSelector.DEFAULT_FRONT_CAMERA else CameraSelector.DEFAULT_BACK_CAMERA
                this.preview?.let { runCatching { p.unbind(it) } }
                val cam = p.bindToLifecycle(this, sel, preview)
                camera = cam
                val ev = cam.cameraInfo.exposureState
                cam.cameraControl.enableTorch(ui.torch && cam.cameraInfo.hasFlashUnit())
                cam.cameraControl.setExposureCompensationIndex(ui.exposure.coerceIn(ev.exposureCompensationRange.lower, ev.exposureCompensationRange.upper))
                cam.cameraControl.setLinearZoom(ui.zoom)
                startOrientation()
                _ui.update { it.copy(streaming = true, error = null, exposureRange = ev.exposureCompensationRange.lower..ev.exposureCompensationRange.upper) }
            } catch (e: Exception) {
                Log.w(TAG, "kamera başlatılamadı: $e")
                fail(R.string.cam_err_open)
            }
        }, ContextCompat.getMainExecutor(this))
    }

    /** Cihaz dönünce Preview hedef dönüşünü güncelle (dikey/yatay tutuş); TransformationInfo ile PC'ye bildirilir. */
    private fun startOrientation() {
        orient?.disable()
        val l = object : OrientationEventListener(this) {
            override fun onOrientationChanged(deg: Int) {
                if (deg == ORIENTATION_UNKNOWN) return
                val r = when {
                    deg >= 315 || deg < 45 -> Surface.ROTATION_0
                    deg < 135 -> Surface.ROTATION_270
                    deg < 225 -> Surface.ROTATION_180
                    else -> Surface.ROTATION_90
                }
                preview?.targetRotation = r
            }
        }
        if (l.canDetectOrientation()) l.enable()
        orient = l
    }

    private fun stopCamera() {
        generation++
        orient?.disable()
        orient = null
        preview?.let { try { provider?.unbind(it) } catch (_: Exception) {} }
        preview = null
        camera = null
        enc?.release()
        enc = null
        Core.node?.endCamStream()
        _ui.update { it.copy(streaming = false) }
        Core.node?.sendCamState(false, 0u, 0u, 30u, if (_ui.value.front) 1u else 0u, _ui.value.mirror, 0u.toUShort())
    }

    private fun fail(@StringRes msg: Int) {
        _ui.update { it.copy(streaming = false, error = msg) }
        stopSelf()
    }

    private fun publishState(w: Int, h: Int) {
        _ui.update { it.copy(w = w, h = h) }
        val u = _ui.value
        Core.node?.sendCamState(true, w.toUInt(), h.toUInt(), 30u, if (u.front) 1u else 0u, u.mirror, u.rotation.toUShort())
    }

    fun control(c: (Camera) -> Unit) {
        camera?.let { runCatching { c(it) } }
    }

    fun keyframe() = enc?.requestKey()
    fun republish() = publishState(_ui.value.w, _ui.value.h)

    // ---------------------------------------------------------------- kodlayıcı

    private class Encoder(val w: Int, val h: Int, fps: Int, private var kbps: Int) {
        private val codec: MediaCodec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        val surface: Surface
        @Volatile private var running = true
        private val codecLock = Any()
        private var csd: ByteArray = ByteArray(0)
        private val th: Thread

        init {
            var createdSurface: Surface? = null
            var lastErr: Exception? = null
            for (full in listOf(true, false)) {
                val f = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, w, h).apply {
                    setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
                    setInteger(MediaFormat.KEY_BIT_RATE, kbps * 1000)
                    setInteger(MediaFormat.KEY_FRAME_RATE, fps)
                    setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2) // GOP ~2 sn
                    if (full) {
                        setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR)
                        setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline)
                        setInteger(MediaFormat.KEY_PRIORITY, 0)
                        if (Build.VERSION.SDK_INT >= 30) setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
                    }
                }
                try {
                    codec.configure(f, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
                    val s = codec.createInputSurface()
                    try {
                        codec.start()
                        createdSurface = s
                        lastErr = null
                        break
                    } catch (e: Exception) {
                        s.release()
                        throw e
                    }
                } catch (e: Exception) {
                    lastErr = e
                    runCatching { codec.reset() }
                }
            }
            if (createdSurface == null) {
                runCatching { codec.release() }
                throw lastErr ?: IllegalStateException("encoder init failed")
            }
            surface = createdSurface
            th = Thread({ drain() }, "cam-encode")
            th.start()
        }

        fun requestKey() {
            synchronized(codecLock) {
                if (!running) return
                try {
                    codec.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0) })
                } catch (_: Exception) {}
            }
        }

        private fun adapt(backlogMs: Long) {
            val target = if (h >= 1000) 6000 else 3000
            val next = when {
                backlogMs > 100 -> (kbps * 0.75).toInt().coerceAtLeast(target / 4)
                backlogMs < 20 && kbps < target -> (kbps * 1.1).toInt() + 1
                else -> kbps
            }.coerceAtMost(target)
            if (kotlin.math.abs(next - kbps) * 20 > kbps) {
                kbps = next
                synchronized(codecLock) {
                    if (!running) return
                    try { codec.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, kbps * 1000) }) } catch (_: Exception) {}
                }
            }
        }

        private fun drain() {
            val info = MediaCodec.BufferInfo()
            var lastAdapt = 0L
            while (running) {
                val i = try { codec.dequeueOutputBuffer(info, 10_000) } catch (_: Exception) { return }
                if (i < 0) continue
                try {
                    val buf = codec.getOutputBuffer(i)
                    if (buf != null && info.size > 0) {
                        buf.position(info.offset)
                        buf.limit(info.offset + info.size)
                        val bytes = ByteArray(info.size)
                        buf.get(bytes)
                        if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) {
                            csd = bytes // SPS+PPS (Annex-B)
                        } else {
                            val key = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
                            val payload = if (key && csd.isNotEmpty()) csd + bytes else bytes
                            val node = Core.node
                            if (node != null) {
                                val sent = node.sendCamFrame(info.presentationTimeUs.toULong(), key, payload)
                                val fb = node.camFeedback(info.presentationTimeUs.toULong())
                                if (!sent || fb.wantKey) requestKey()
                                val now = System.currentTimeMillis()
                                if (now - lastAdapt > 500) { lastAdapt = now; adapt(fb.backlogMs.toLong()) }
                            }
                        }
                    }
                } finally {
                    try { codec.releaseOutputBuffer(i, false) } catch (_: Exception) {}
                }
            }
        }

        @Synchronized fun release() {
            if (!running) return
            running = false
            if (Thread.currentThread() !== th) {
                runCatching { th.join(500) }
            }
            synchronized(codecLock) {
                try { codec.stop() } catch (_: Exception) {}
                try { codec.release() } catch (_: Exception) {}
                try { surface.release() } catch (_: Exception) {}
            }
        }
    }

    companion object {
        private const val TAG = "kopru-cam"
        private const val CH = "kopru_camera"
        private const val ID = 4201

        @Volatile private var instance: CameraService? = null
        private val _ui = MutableStateFlow(CamUi())
        val ui: StateFlow<CamUi> = _ui

        fun supported(): Boolean = Core.node?.peerSupportsCamera() == true

        fun permissionDenied() {
            _ui.update { it.copy(error = R.string.cam_err_permission) }
        }

        fun start(ctx: Context) {
            if (instance != null) return
            try {
                _ui.update { it.copy(error = null) }
                ctx.startForegroundService(Intent(ctx, CameraService::class.java))
            } catch (e: SecurityException) {
                Log.w(TAG, "kamera hizmeti izni yok: $e")
                _ui.update { it.copy(error = R.string.cam_err_permission) }
            } catch (e: IllegalStateException) {
                Log.w(TAG, "kamera hizmeti başlatılamadı: $e")
                _ui.update { it.copy(error = R.string.cam_err_open) }
            }
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, CameraService::class.java))
        }

        /** Çözünürlük/kamera değişimi: yayın sürüyorsa yeniden kurulur, ardından PC'ye yeni durum bildirilir. */
        fun setRes(r: Int) {
            _ui.update { it.copy(res = r) }
            instance?.restart()
        }

        fun setFront(f: Boolean) {
            _ui.update { it.copy(front = f) }
            instance?.restart()
        }

        fun setMirror(m: Boolean) {
            _ui.update { it.copy(mirror = m) }
            instance?.republish()
        }

        fun setTorch(t: Boolean) {
            _ui.update { it.copy(torch = t) }
            instance?.control { c -> if (c.cameraInfo.hasFlashUnit()) c.cameraControl.enableTorch(t) }
        }

        fun setExposure(i: Int) {
            _ui.update { it.copy(exposure = i) }
            instance?.control { c -> c.cameraControl.setExposureCompensationIndex(i) }
        }

        fun setZoom(z: Float) {
            _ui.update { it.copy(zoom = z) }
            instance?.control { c -> c.cameraControl.setLinearZoom(z) }
        }

        /** PC komutları (sanal kamera açıldı/biçim istendi/anahtar kare). */
        fun onCmd(ctx: Context, cmd: CamCmd) {
            when (cmd) {
                is CamCmd.Keyframe -> instance?.keyframe()
                is CamCmd.Stop -> stop(ctx)
                is CamCmd.Start -> {
                    val want = if (cmd.h.toInt() >= 1000) 1080 else 720
                    if (instance != null && want != _ui.value.res) setRes(want) else instance?.keyframe()
                }
            }
        }

        fun onDisconnected(ctx: Context) {
            if (instance != null) stop(ctx)
        }
    }
}
