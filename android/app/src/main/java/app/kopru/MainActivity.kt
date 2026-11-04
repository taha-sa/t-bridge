package app.kopru

import android.content.ClipboardManager
import android.content.Intent
import android.widget.Toast
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Mouse
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.ui.FilesScreen
import app.kopru.ui.HomeScreen
import app.kopru.ui.KopruTheme
import app.kopru.ui.RemoteScreen

class MainActivity : ComponentActivity() {
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener { sendClipIfNew() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        KopruService.start(this)
        Core.start()
        handlePairLink(intent)
        setContent {
            KopruTheme {
                val ui by Core.ui.collectAsStateWithLifecycle()
                var tab by rememberSaveable { mutableIntStateOf(0) }
                Scaffold(
                    bottomBar = {
                        NavigationBar {
                            NavigationBarItem(tab == 0, { tab = 0 }, { Icon(Icons.Outlined.Home, null) }, label = { Text("Bağlantı") })
                            NavigationBarItem(tab == 1, { tab = 1 }, { Icon(Icons.Outlined.Folder, null) }, label = { Text("Dosyalar") })
                            NavigationBarItem(tab == 2, { tab = 2 }, { Icon(Icons.Outlined.Mouse, null) }, label = { Text("Kumanda") })
                        }
                    },
                ) { pad ->
                    Box(Modifier.fillMaxSize().padding(pad)) {
                        when (tab) {
                            0 -> HomeScreen(ui)
                            1 -> FilesScreen(ui)
                            else -> RemoteScreen(ui)
                        }
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
        if (data.scheme != "kopru" || data.host != "pair") return
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

    private fun sendClipIfNew() {
        val cm = getSystemService(ClipboardManager::class.java) ?: return
        val text = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString() ?: return
        if (text == Core.lastClipSet) return
        Core.sendClipboardText(text)
    }
}
