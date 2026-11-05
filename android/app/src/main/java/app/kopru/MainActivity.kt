package app.kopru

import android.content.ClipboardManager
import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Mouse
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.ui.FilesScreen
import app.kopru.ui.HomeScreen
import app.kopru.ui.RemoteScreen
import app.kopru.ui.SettingsScreen
import app.kopru.ui.TBridgeTheme
import app.kopru.ui.tb

class MainActivity : ComponentActivity() {
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener { sendClipIfNew() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        KopruService.start(this)
        Core.start()
        handlePairLink(intent)
        setContent {
            TBridgeTheme {
                val ui by Core.ui.collectAsStateWithLifecycle()
                var tab by rememberSaveable { mutableIntStateOf(0) }
                val c = tb
                Box(
                    Modifier
                        .fillMaxSize()
                        .background(c.bg)
                        .background(
                            Brush.radialGradient(
                                listOf(c.gold.copy(alpha = if (c.dark) 0.10f else 0.08f), Color.Transparent),
                                center = Offset(900f, -120f),
                                radius = 1100f,
                            ),
                        ),
                ) {
                    Column(Modifier.fillMaxSize()) {
                        Box(Modifier.weight(1f).statusBarsPadding()) {
                            AnimatedContent(
                                tab,
                                transitionSpec = {
                                    (fadeIn(tween(220)) + slideInVertically(tween(260)) { it / 30 }) togetherWith fadeOut(tween(120))
                                },
                                label = "tab",
                            ) { t ->
                                when (t) {
                                    0 -> HomeScreen(ui, go = { tab = it })
                                    1 -> FilesScreen(ui)
                                    2 -> RemoteScreen(ui)
                                    else -> SettingsScreen(ui)
                                }
                            }
                        }
                        NavBar(tab, active = ui.transfers.values.count { it.state == uniffi.kopru_core.TransferState.ACTIVE }) { tab = it }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handlePairLink(intent)
    }

    private fun handlePairLink(i: Intent?) {
        val data = i?.data ?: return
        if ((data.scheme != "tbridge" && data.scheme != "kopru") || data.host != "pair") return
        i.data = null
        runCatching { Core.node?.pairUri(data.toString()) }
            .onFailure { Toast.makeText(this, "Geçersiz eşleştirme bağlantısı", Toast.LENGTH_LONG).show() }
    }

    override fun onResume() {
        super.onResume()
        getSystemService(ClipboardManager::class.java)?.addPrimaryClipChangedListener(clipListener)
        Core.node?.nudge()
    }

    override fun onPause() {
        getSystemService(ClipboardManager::class.java)?.removePrimaryClipChangedListener(clipListener)
        super.onPause()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // Uygulama ön plandayken pano okunabilir: başka uygulamada kopyalanıp gelindiyse otomatik gönder.
        if (hasFocus) sendClipIfNew()
    }

    private fun sendClipIfNew(): Boolean {
        val cm = getSystemService(ClipboardManager::class.java) ?: return false
        val text = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString() ?: return false
        if (text == Core.lastClipSet) return false
        return Core.sendClipboardText(text)
    }
}

private data class NavItem(val label: String, val icon: ImageVector)

private val items = listOf(
    NavItem("Bağlantı", Icons.Outlined.Home),
    NavItem("Dosyalar", Icons.Outlined.Folder),
    NavItem("Kumanda", Icons.Outlined.Mouse),
    NavItem("Ayarlar", Icons.Outlined.Settings),
)

/** Alt gezinme: seçili sekmede altın gösterge. */
@Composable
private fun NavBar(selected: Int, active: Int, onSelect: (Int) -> Unit) {
    val c = tb
    Column(
        Modifier
            .fillMaxWidth()
            .background(c.bg)
            .windowInsetsPadding(WindowInsets.navigationBars),
    ) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
        Row(Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp), horizontalArrangement = Arrangement.SpaceAround) {
            items.forEachIndexed { i, it ->
                val sel = i == selected
                val w by animateDpAsState(if (sel) 22.dp else 0.dp, tween(260), label = "ind")
                Column(
                    Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(16.dp))
                        .clickable(remember { MutableInteractionSource() }, indication = null) { onSelect(i) }
                        .padding(vertical = 6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(contentAlignment = Alignment.TopEnd) {
                        Box(
                            Modifier
                                .clip(RoundedCornerShape(12.dp))
                                .background(if (sel) c.goldSoft else Color.Transparent)
                                .padding(horizontal = 16.dp, vertical = 5.dp),
                        ) {
                            Icon(it.icon, it.label, tint = if (sel) c.goldHi else c.muted, modifier = Modifier.size(22.dp))
                        }
                        if (i == 1 && active > 0) {
                            Box(Modifier.padding(top = 2.dp, end = 10.dp).size(8.dp).clip(RoundedCornerShape(4.dp)).background(c.goldHi))
                        }
                    }
                    Spacer(Modifier.height(4.dp))
                    Text(it.label, color = if (sel) c.text else c.muted, style = MaterialTheme.typography.labelMedium)
                    Spacer(Modifier.height(3.dp))
                    Box(Modifier.width(w).height(2.dp).clip(RoundedCornerShape(1.dp)).background(c.goldBrush))
                }
            }
        }
    }
}
