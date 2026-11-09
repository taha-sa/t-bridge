package app.kopru.ui

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Videocam
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.CameraService
import app.kopru.UiState
import uniffi.kopru_core.ConnState
import kotlin.math.roundToInt

/**
 * Kamera sekmesi: telefonu bilgisayarın web kamerası yap. PC `camera` yeteneğini duyurmuyorsa (Windows 10 / eski sürüm)
 * kilitli kart gösterilir.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CameraTab(ui: UiState) {
    val c = tb
    val ctx = LocalContext.current
    val cam by CameraService.ui.collectAsStateWithLifecycle()
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
        granted = ok
        if (ok) CameraService.start(ctx)
    }
    val on = ui.conn == ConnState.CONNECTED
    val supported = on && CameraService.supported()

    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Kamera", Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Pill(if (on) "canlı" else "bağlı değil", if (on) "violet" else "muted")
        }

        if (!supported) {
            // Kilitli durum: Windows 10 veya eski PC sürümü (ya da bağlı değil)
            TbCard {
                Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    IconTile(Icons.Outlined.Lock, size = 64.dp)
                    Spacer(Modifier.height(4.dp))
                    Text(if (on) "Windows 11 gerekli" else "Bağlı değil", color = c.text, style = MaterialTheme.typography.titleMedium)
                    Text(
                        if (on) "Bu özellik yalnızca Windows 11 için tasarlanmıştır." else "Bağlanınca telefonu bilgisayarda web kamerası olarak kullanabilirsin.",
                        color = c.muted,
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
            return@Column
        }

        TbCard {
            Text(
                "Telefonunu bilgisayarın web kamerası yap. Görüntü yalnızca bilgisayarına, şifreli bağlantıyla gider. Kamera izni gerekir; yayın sürerken bildirim görünür.",
                color = c.muted,
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(16.dp))
            if (cam.error != null) {
                Text(cam.error!!, color = c.bad, style = MaterialTheme.typography.bodySmall)
                Spacer(Modifier.height(8.dp))
            }
            GoldButton(
                if (cam.streaming) "Yayını durdur" else "Yayını başlat",
                {
                    if (cam.streaming) CameraService.stop(ctx)
                    else if (granted) CameraService.start(ctx)
                    else ask.launch(Manifest.permission.CAMERA)
                },
                Modifier.fillMaxWidth(),
                icon = Icons.Outlined.Videocam,
                height = 56.dp,
            )
            if (cam.streaming) {
                Spacer(Modifier.height(8.dp))
                Text("Yayında · ${cam.w}×${cam.h}", color = c.goldHi, style = MaterialTheme.typography.labelLarge)
            }
        }

        TbCard {
            SectionTitle("Ayarlar")
            Spacer(Modifier.height(12.dp))
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip(if (cam.front) "Ön kamera ✓" else "Ön kamera") { CameraService.setFront(true) }
                Chip(if (!cam.front) "Arka kamera ✓" else "Arka kamera") { CameraService.setFront(false) }
                Chip(if (cam.res == 720) "720p ✓" else "720p") { CameraService.setRes(720) }
                Chip(if (cam.res == 1080) "1080p ✓" else "1080p") { CameraService.setRes(1080) }
                Chip(if (cam.mirror) "Ayna ✓" else "Ayna") { CameraService.setMirror(!cam.mirror) }
                Chip(if (cam.torch) "Fener ✓" else "Fener") { CameraService.setTorch(!cam.torch) }
            }
            if (cam.exposureRange.last > cam.exposureRange.first) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("Pozlama", Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
                    Slider(
                        cam.exposure.toFloat(),
                        { CameraService.setExposure(it.roundToInt()) },
                        valueRange = cam.exposureRange.first.toFloat()..cam.exposureRange.last.toFloat(),
                        modifier = Modifier.weight(1f),
                        colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
                    )
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Yakınlık", Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
                Slider(
                    cam.zoom,
                    { CameraService.setZoom(it) },
                    valueRange = 0f..1f,
                    modifier = Modifier.weight(1f),
                    colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
                )
            }
            Text("Telefonu dikey ya da yatay tutabilirsin; bilgisayara her zaman dik görüntü gider.", color = c.faint, style = MaterialTheme.typography.bodySmall)
        }
    }
}
