package app.kopru

import android.content.ComponentName
import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.view.KeyEvent
import uniffi.kopru_core.MediaAction
import uniffi.kopru_core.NowPlaying
import uniffi.kopru_core.VolumeAction

/** "Telefonumu bul": sessiz moddayken bile alarm kanalından tam seste çalar, 30 sn sonra durur. */
object Ringer {
    private var player: MediaPlayer? = null
    private var savedVolume = -1
    private val main = Handler(Looper.getMainLooper())
    private val autoStop = Runnable { stop() }

    fun start(ctx: Context) {
        if (player != null) return
        val am = ctx.getSystemService(AudioManager::class.java)
        savedVolume = am.getStreamVolume(AudioManager.STREAM_ALARM)
        // Rahatsız Etmeyin açıkken ses değiştirmek SecurityException atabilir: çökmesin, mevcut sesle çalsın.
        runCatching { am.setStreamVolume(AudioManager.STREAM_ALARM, am.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0) }
        val uri = RingtoneManager.getActualDefaultRingtoneUri(ctx, RingtoneManager.TYPE_ALARM)
            ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
        player = runCatching {
            MediaPlayer().apply {
                setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
                setDataSource(ctx, uri)
                isLooping = true
                prepare()
                start()
            }
        }.getOrNull()
        runCatching { ctx.getSystemService(Vibrator::class.java)?.vibrate(VibrationEffect.createWaveform(longArrayOf(0, 600, 400), 0)) }
        KopruService.showRinging(ctx, true)
        main.removeCallbacks(autoStop)
        main.postDelayed(autoStop, 30_000)
    }

    fun stop() {
        main.removeCallbacks(autoStop)
        val ctx = KopruService.appCtx ?: return
        player?.runCatching { stop(); release() }
        player = null
        ctx.getSystemService(Vibrator::class.java)?.cancel()
        if (savedVolume >= 0) {
            runCatching { ctx.getSystemService(AudioManager::class.java).setStreamVolume(AudioManager.STREAM_ALARM, savedVolume, 0) }
            savedVolume = -1
        }
        KopruService.showRinging(ctx, false)
    }

    val ringing get() = player != null
}

/** Telefonun medya oturumları: PC'den kontrol + çalan parça bilgisini PC'ye gönderme. */
object MediaControl {
    private var controller: MediaController? = null
    private var last: NowPlaying? = null
    private val callback = object : MediaController.Callback() {
        override fun onMetadataChanged(metadata: android.media.MediaMetadata?) = publish()
        override fun onPlaybackStateChanged(state: PlaybackState?) = publish()
        override fun onSessionDestroyed() {
            controller = null
            publish()
        }
    }

    /** NotifListener bağlanınca çağrılır (oturum listesi bildirim erişimi ister). */
    fun attach(ctx: Context) {
        val msm = ctx.getSystemService(MediaSessionManager::class.java) ?: return
        val comp = ComponentName(ctx, NotifListener::class.java)
        runCatching {
            msm.addOnActiveSessionsChangedListener({ list -> pick(list) }, comp)
            pick(msm.getActiveSessions(comp))
        }
    }

    private fun pick(list: List<MediaController>?) {
        val best = list?.firstOrNull { it.playbackState?.state == PlaybackState.STATE_PLAYING } ?: list?.firstOrNull()
        if (best?.sessionToken == controller?.sessionToken) return
        controller?.unregisterCallback(callback)
        controller = best
        best?.registerCallback(callback, Handler(Looper.getMainLooper()))
        publish()
    }

    private fun current(ctx: Context?): NowPlaying? {
        val c = controller ?: return null
        val md = c.metadata ?: return null
        val title = md.getString(android.media.MediaMetadata.METADATA_KEY_TITLE) ?: return null
        val artist = md.getString(android.media.MediaMetadata.METADATA_KEY_ARTIST)
            ?: md.getString(android.media.MediaMetadata.METADATA_KEY_ALBUM_ARTIST) ?: ""
        val app = ctx?.let { NotifListener.appLabel(it, c.packageName) } ?: c.packageName
        return NowPlaying(title, artist, c.playbackState?.state == PlaybackState.STATE_PLAYING, app)
    }

    fun publish(force: Boolean = false) {
        val np = current(KopruService.appCtx)
        if (!force && np == last) return
        last = np
        Core.node?.sendNowPlaying(np)
    }

    fun media(ctx: Context, a: MediaAction) {
        val tc = controller?.transportControls
        if (tc != null) {
            when (a) {
                MediaAction.PLAY_PAUSE -> if (controller?.playbackState?.state == PlaybackState.STATE_PLAYING) tc.pause() else tc.play()
                MediaAction.NEXT -> tc.skipToNext()
                MediaAction.PREVIOUS -> tc.skipToPrevious()
                MediaAction.STOP -> tc.stop()
            }
            return
        }
        // Oturum yoksa sistem medya tuşu.
        val code = when (a) {
            MediaAction.PLAY_PAUSE -> KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE
            MediaAction.NEXT -> KeyEvent.KEYCODE_MEDIA_NEXT
            MediaAction.PREVIOUS -> KeyEvent.KEYCODE_MEDIA_PREVIOUS
            MediaAction.STOP -> KeyEvent.KEYCODE_MEDIA_STOP
        }
        val am = ctx.getSystemService(AudioManager::class.java)
        am.dispatchMediaKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, code))
        am.dispatchMediaKeyEvent(KeyEvent(KeyEvent.ACTION_UP, code))
    }

    fun volume(ctx: Context, a: VolumeAction) {
        val am = ctx.getSystemService(AudioManager::class.java)
        val dir = when (a) {
            VolumeAction.UP -> AudioManager.ADJUST_RAISE
            VolumeAction.DOWN -> AudioManager.ADJUST_LOWER
            VolumeAction.MUTE -> AudioManager.ADJUST_TOGGLE_MUTE
        }
        am.adjustStreamVolume(AudioManager.STREAM_MUSIC, dir, AudioManager.FLAG_SHOW_UI)
    }
}
