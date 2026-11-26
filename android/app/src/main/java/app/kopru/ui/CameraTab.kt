package app.kopru.ui

import androidx.annotation.StringRes
import app.kopru.R
import androidx.compose.ui.res.stringResource
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
import androidx.compose.material3.FilterChip
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
        if (ok) CameraService.start(ctx) else CameraService.permissionDenied()
    }
    val on = ui.conn == ConnState.CONNECTED
    val supported = on && CameraService.supported()

    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.cam_title), Modifier.weight(1f), color = c.text, style = MaterialTheme.typography.headlineMedium)
            Pill(if (on) stringResource(R.string.remote_pill_live) else stringResource(R.string.remote_pill_offline), if (on) "violet" else "muted")
        }

        if (!supported && !cam.streaming) {
            // Kilitli durum: Windows 10 veya eski PC sürümü (ya da bağlı değil)
            TbCard {
                Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    IconTile(Icons.Outlined.Lock, size = 64.dp)
                    Spacer(Modifier.height(4.dp))
                    Text(stringResource(if (on) R.string.cam_win11_required else R.string.cam_not_connected), color = c.text, style = MaterialTheme.typography.titleMedium)
                    Text(
                        stringResource(if (on) R.string.cam_win11_only else R.string.cam_connect_hint),
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
                stringResource(R.string.cam_intro),
                color = c.muted,
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(16.dp))
            if (cam.error != null) {
                Text(stringResource(cam.error!!), color = c.bad, style = MaterialTheme.typography.bodySmall)
                Spacer(Modifier.height(8.dp))
            }
            GoldButton(
                stringResource(if (cam.streaming) R.string.cam_stop else R.string.cam_start),
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
                Text(stringResource(R.string.cam_live, cam.w, cam.h), color = c.goldHi, style = MaterialTheme.typography.labelLarge)
            }
        }

        TbCard {
            SectionTitle(stringResource(R.string.cam_settings))
            Spacer(Modifier.height(12.dp))
            Text(stringResource(R.string.cam_group_camera), color = c.text2, style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                FilterChip(selected = cam.front, onClick = { CameraService.setFront(true) }, label = { Text(stringResource(R.string.cam_front) + if (cam.front) " ✓" else "") })
                FilterChip(selected = !cam.front, onClick = { CameraService.setFront(false) }, label = { Text(stringResource(R.string.cam_back) + if (!cam.front) " ✓" else "") })
            }
            Spacer(Modifier.height(8.dp))
            Text(stringResource(R.string.cam_group_quality), color = c.text2, style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                FilterChip(selected = cam.res == 720, onClick = { CameraService.setRes(720) }, label = { Text("720p" + if (cam.res == 720) " ✓" else "") })
                FilterChip(selected = cam.res == 1080, onClick = { CameraService.setRes(1080) }, label = { Text("1080p" + if (cam.res == 1080) " ✓" else "") })
            }
            Spacer(Modifier.height(8.dp))
            Text(stringResource(R.string.cam_group_image), color = c.text2, style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                FilterChip(selected = cam.mirror, onClick = { CameraService.setMirror(!cam.mirror) }, label = { Text(stringResource(R.string.cam_mirror) + if (cam.mirror) " ✓" else "") })
                FilterChip(selected = cam.torch, onClick = { CameraService.setTorch(!cam.torch) }, label = { Text(stringResource(R.string.cam_torch) + if (cam.torch) " ✓" else "") })
            }
            if (cam.exposureRange.last > cam.exposureRange.first) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.cam_exposure), Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
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
                Text(stringResource(R.string.cam_zoom), Modifier.width(76.dp), color = c.muted, style = MaterialTheme.typography.labelLarge)
                Slider(
                    cam.zoom,
                    { CameraService.setZoom(it) },
                    valueRange = 0f..1f,
                    modifier = Modifier.weight(1f),
                    colors = SliderDefaults.colors(thumbColor = c.goldHi, activeTrackColor = c.gold, inactiveTrackColor = c.s4),
                )
            }
            Text(stringResource(R.string.cam_orientation), color = c.faint, style = MaterialTheme.typography.bodySmall)
        }
    }
}
