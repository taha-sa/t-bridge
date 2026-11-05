package app.kopru.ui

import android.Manifest
import android.content.pm.PackageManager
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors

/** Eşleştirme bağlantısı mı? Başka QR kodlar okunmuş sayılmaz, tarama sürer. */
private fun isPairUri(s: String) = s.startsWith("tbridge://pair?") || s.startsWith("kopru://pair?")

/** Görüntünün parlaklık düzleminden QR çözer; bulamazsa null. */
private class QrDecoder {
    private val reader = MultiFormatReader().apply {
        setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE), DecodeHintType.TRY_HARDER to true))
    }

    fun decode(img: ImageProxy): String? {
        val plane = img.planes[0]
        val buf = plane.buffer
        val w = img.width
        val h = img.height
        val stride = plane.rowStride
        val data = ByteArray(w * h)
        // Satır adımı genişlikten büyük olabilir: yalnızca görünür baytları kopyala.
        for (row in 0 until h) {
            buf.position(row * stride)
            buf.get(data, row * w, w)
        }
        val src = PlanarYUVLuminanceSource(data, w, h, 0, 0, w, h, false)
        return try {
            reader.decodeWithState(BinaryBitmap(HybridBinarizer(src))).text
        } catch (_: NotFoundException) {
            null
        } finally {
            reader.reset()
        }
    }
}

/** Tam ekran, uygulama içi QR okuyucu (Play Hizmetleri gerektirmez). */
@Composable
fun QrScanner(onClose: () -> Unit, onResult: (String) -> Unit) {
    val ctx = LocalContext.current
    var granted by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    var denied by remember { mutableStateOf(false) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        denied = !it
    }
    LaunchedEffect(Unit) { if (!granted) ask.launch(Manifest.permission.CAMERA) }

    Dialog(onClose, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        BackHandler(onBack = onClose)
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            if (granted) {
                CameraPreview(onClose, onResult)
                Viewfinder()
            }
            Column(
                Modifier.fillMaxSize().padding(horizontal = 24.dp, vertical = 48.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    if (denied) "Kamera izni verilmedi" else "Bilgisayardaki QR kodu çerçeveye getir",
                    color = Color.White,
                    style = MaterialTheme.typography.titleMedium,
                    textAlign = TextAlign.Center,
                )
                Column(Modifier.fillMaxWidth()) {
                    if (denied) {
                        Text(
                            "QR okutmak için kameraya izin ver ya da listeden bilgisayarı seçip 6 haneli kodu gir.",
                            Modifier.fillMaxWidth(),
                            color = Color(0xFFC9C6BD),
                            style = MaterialTheme.typography.bodyMedium,
                            textAlign = TextAlign.Center,
                        )
                        Spacer(Modifier.height(16.dp))
                    }
                    TbButton("Vazgeç", onClose, Modifier.fillMaxWidth())
                }
            }
        }
    }
}

@Composable
private fun CameraPreview(onClose: () -> Unit, onResult: (String) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val result by rememberUpdatedState(onResult)
    val close by rememberUpdatedState(onClose)
    val view = remember {
        PreviewView(ctx).apply {
            // Dialog içinde SurfaceView üstte kalan arayüzle çakışabilir; TextureView güvenli.
            implementationMode = PreviewView.ImplementationMode.COMPATIBLE
            scaleType = PreviewView.ScaleType.FILL_CENTER
        }
    }
    DisposableEffect(owner) {
        val exec = Executors.newSingleThreadExecutor()
        val main = ContextCompat.getMainExecutor(ctx)
        val future = ProcessCameraProvider.getInstance(ctx)
        val decoder = QrDecoder()
        var done = false
        var disposed = false
        future.addListener({
            if (disposed) return@addListener
            try {
                val provider = future.get()
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(exec) { img ->
                    val text = try {
                        if (done) null else decoder.decode(img)
                    } catch (_: Exception) {
                        null
                    } finally {
                        img.close()
                    }
                    if (text != null && isPairUri(text) && !done) {
                        done = true
                        main.execute { result(text) }
                    }
                }
                provider.unbindAll()
                provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
            } catch (_: Exception) {
                Toast.makeText(ctx, "Kamera açılamadı. Listeden bilgisayarı seçip kodu gir.", Toast.LENGTH_LONG).show()
                close()
            }
        }, main)
        onDispose {
            disposed = true
            done = true
            if (future.isDone) runCatching { future.get().unbindAll() }
            exec.shutdown()
        }
    }
    AndroidView({ view }, Modifier.fillMaxSize())
}

/** Karartılmış çevre + altın köşeli çerçeve. */
@Composable
private fun Viewfinder() {
    val c = tb
    Canvas(Modifier.fillMaxSize().graphicsLayer(compositingStrategy = CompositingStrategy.Offscreen)) {
        val side = size.minDimension * 0.68f
        val tl = Offset((size.width - side) / 2, (size.height - side) / 2)
        val r = 24.dp.toPx()
        drawRect(Color(0xB3000000))
        drawRoundRect(Color.Black, tl, Size(side, side), CornerRadius(r), blendMode = BlendMode.Clear)
        val len = side * 0.16f
        val w = 4.dp.toPx()
        val g = c.goldHi
        val br = Offset(tl.x + side, tl.y + side)
        val st = Stroke(w, cap = StrokeCap.Round)
        drawArc(g, 180f, 90f, false, tl, Size(r * 2, r * 2), style = st)
        drawLine(g, Offset(tl.x + r, tl.y), Offset(tl.x + len, tl.y), w, StrokeCap.Round)
        drawLine(g, Offset(tl.x, tl.y + r), Offset(tl.x, tl.y + len), w, StrokeCap.Round)
        drawArc(g, 270f, 90f, false, Offset(br.x - r * 2, tl.y), Size(r * 2, r * 2), style = st)
        drawLine(g, Offset(br.x - r, tl.y), Offset(br.x - len, tl.y), w, StrokeCap.Round)
        drawLine(g, Offset(br.x, tl.y + r), Offset(br.x, tl.y + len), w, StrokeCap.Round)
        drawArc(g, 90f, 90f, false, Offset(tl.x, br.y - r * 2), Size(r * 2, r * 2), style = st)
        drawLine(g, Offset(tl.x + r, br.y), Offset(tl.x + len, br.y), w, StrokeCap.Round)
        drawLine(g, Offset(tl.x, br.y - r), Offset(tl.x, br.y - len), w, StrokeCap.Round)
        drawArc(g, 0f, 90f, false, Offset(br.x - r * 2, br.y - r * 2), Size(r * 2, r * 2), style = st)
        drawLine(g, Offset(br.x - r, br.y), Offset(br.x - len, br.y), w, StrokeCap.Round)
        drawLine(g, Offset(br.x, br.y - r), Offset(br.x, br.y - len), w, StrokeCap.Round)
    }
}
