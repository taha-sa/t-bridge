package app.kopru

import android.content.ClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.annotation.StringRes
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
import androidx.compose.material.icons.outlined.Cast
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected as selectedState
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Folder
import androidx.compose.material.icons.outlined.MoreHoriz
import androidx.compose.material.icons.outlined.Videocam
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Hub
import androidx.compose.material.icons.outlined.Mouse
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
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
import app.kopru.ui.CameraTab
import app.kopru.ui.Chrome
import app.kopru.ui.RemoteScreen
import app.kopru.ui.ScreenTab
import app.kopru.ui.SettingsScreen
import app.kopru.ui.TBridgeTheme
import app.kopru.ui.tb

@OptIn(ExperimentalLayoutApi::class)
class MainActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: android.content.Context) = super.attachBaseContext(Lang.wrap(newBase))

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
                val imeUp = WindowInsets.isImeVisible
                // Tam ekran (Ekran sekmesi): sistem çubukları gizlenir, alt gezinme kalkar. Sekmeden çıkınca kapanır.
                LaunchedEffect(tab) { if (tab != TAB_SCREEN) Chrome.immersive = false }
                LaunchedEffect(Chrome.immersive) {
                    val ctl = WindowCompat.getInsetsController(window, window.decorView)
                    if (Chrome.immersive) {
                        ctl.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                        ctl.hide(WindowInsetsCompat.Type.systemBars())
                    } else {
                        ctl.show(WindowInsetsCompat.Type.systemBars())
                    }
                }
                BackHandler(Chrome.immersive) { Chrome.immersive = false }
                app.kopru.ui.CrashPrompt()
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
                    // imePadding: yazı alanı açılınca içerik klavyenin üstünde kalır (Ekran'da görüntü küçülür, durmaz).
                    Column(Modifier.fillMaxSize().imePadding()) {
                        // Ekran sekmesi tam taşar (görüntü durum çubuğunun altına da uzanır, kendi boşluğunu yönetir).
                        Box(Modifier.weight(1f).then(if (tab == TAB_SCREEN) Modifier else Modifier.statusBarsPadding())) {
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
                                    TAB_SCREEN -> ScreenTab(ui)
                                    3 -> RemoteScreen(ui)
                                    4 -> CameraTab(ui)
                                    5 -> app.kopru.ui.EcosystemScreen(ui)
                                    else -> SettingsScreen(ui)
                                }
                            }
                        }
                        if (!Chrome.immersive && !imeUp) {
                            NavBar(tab, active = ui.transfers.values.count { it.state == uniffi.kopru_core.TransferState.ACTIVE }) { tab = it }
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
        if ((data.scheme != "tbridge" && data.scheme != "kopru") || data.host != "pair") return
        i.data = null
        val uriStr = data.toString()
        val label = data.getQueryParameter("n")?.takeIf { it.isNotBlank() }?.take(80)
            ?: getString(R.string.svc_notif_default_pc)
        android.app.AlertDialog.Builder(this)
            .setTitle(getString(R.string.app_name))
            .setMessage(label)
            .setPositiveButton(android.R.string.ok) { _, _ ->
                runCatching { Core.node?.pairUri(uriStr) }
                    .onFailure { Toast.makeText(this, getString(R.string.main_err_pair_link), Toast.LENGTH_LONG).show() }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    override fun onResume() {
        super.onResume()
        getSystemService(ClipboardManager::class.java)?.addPrimaryClipChangedListener(clipListener)
        Core.rescan()
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

private const val TAB_SCREEN = 2

private data class NavItem(val tab: Int, @StringRes val label: Int, @StringRes val desc: Int, val icon: ImageVector)

/*
 * Alt gezinme düzeni: 7 hedef var ama telefon genişliğinde (360 dp) 7 sekme ancak ~51 dp/sekme olur; "Ekosistem" gibi
 * etiketler sığmaz ve 48 dp dokunma hedefi korunamaz. Bu yüzden 5 yuva: Ana · Dosyalar · Ekran · Kumanda birincil (en sık
 * kullanılan, telefondan PC denetimi öncelikli), 5. yuva "Daha fazla" alt sayfası (Kamera · Ekosistem · Ayarlar: seyrek,
 * bir kez kurulan şeyler). Kamera/Ekran/Kumanda ayrı hedeflerdir; "Daha fazla" içindeki bir hedefteyken yuva seçili görünür.
 */
private val primary = listOf(
    NavItem(0, R.string.nav_home, R.string.nav_home_desc, Icons.Outlined.Home),
    NavItem(1, R.string.nav_files, R.string.nav_files_desc, Icons.Outlined.Folder),
    NavItem(TAB_SCREEN, R.string.nav_screen, R.string.nav_screen_desc, Icons.Outlined.Cast),
    NavItem(3, R.string.nav_remote, R.string.nav_remote_desc, Icons.Outlined.Mouse),
)
private val more = listOf(
    NavItem(4, R.string.nav_camera, R.string.nav_camera_desc, Icons.Outlined.Videocam),
    NavItem(5, R.string.nav_eco, R.string.nav_eco_desc, Icons.Outlined.Hub),
    NavItem(6, R.string.nav_settings, R.string.nav_settings_desc, Icons.Outlined.Settings),
)

/** Alt gezinme: seçili sekmede altın gösterge. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun NavBar(selected: Int, active: Int, onSelect: (Int) -> Unit) {
    val c = tb
    var sheet by remember { mutableStateOf(false) }
    val slots = primary + NavItem(-1, R.string.nav_more, R.string.nav_more_desc, Icons.Outlined.MoreHoriz)
    Column(
        Modifier
            .fillMaxWidth()
            .background(c.bg)
            .windowInsetsPadding(WindowInsets.navigationBars),
    ) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(c.line))
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp), horizontalArrangement = Arrangement.SpaceAround) {
            slots.forEach { it ->
                val navDesc = stringResource(it.label) + ", " + stringResource(it.desc)
                val sel = if (it.tab < 0) selected >= 4 else it.tab == selected
                val w by animateDpAsState(if (sel) 22.dp else 0.dp, tween(260), label = "ind")
                Column(
                    Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(16.dp))
                        .semantics(mergeDescendants = true) {
                            role = Role.Tab
                            selectedState = sel
                            contentDescription = navDesc
                        }
                        .clickable(remember { MutableInteractionSource() }, indication = null) { if (it.tab < 0) sheet = true else onSelect(it.tab) }
                        .heightIn(min = 48.dp)
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
                            Icon(it.icon, null, tint = if (sel) c.goldHi else c.muted, modifier = Modifier.size(22.dp))
                        }
                        if (it.tab == 1 && active > 0) {
                            Box(Modifier.padding(top = 2.dp, end = 10.dp).size(8.dp).clip(RoundedCornerShape(4.dp)).background(c.goldHi))
                        }
                    }
                    Spacer(Modifier.height(4.dp))
                    Text(stringResource(it.label), color = if (sel) c.text else c.muted, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                    Spacer(Modifier.height(3.dp))
                    Box(Modifier.width(w).height(2.dp).clip(RoundedCornerShape(1.dp)).background(c.goldBrush))
                }
            }
        }
    }
    if (sheet) {
        ModalBottomSheet(onDismissRequest = { sheet = false }, sheetState = rememberModalBottomSheetState(), containerColor = c.s1) {
            Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.nav_more), color = c.text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(bottom = 4.dp))
                more.forEach { m ->
                    val sel = m.tab == selected
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(16.dp))
                            .background(if (sel) c.goldSoft else c.s2)
                            .clickable { sheet = false; onSelect(m.tab) }
                            .heightIn(min = 64.dp)
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(m.icon, null, tint = if (sel) c.goldHi else c.text2, modifier = Modifier.size(24.dp))
                        Spacer(Modifier.width(16.dp))
                        Column(Modifier.weight(1f)) {
                            Text(stringResource(m.label), color = c.text, style = MaterialTheme.typography.titleSmall)
                            Text(stringResource(m.desc), color = c.muted, style = MaterialTheme.typography.bodySmall)
                        }
                        if (sel) Icon(Icons.Outlined.Check, stringResource(R.string.nav_selected), tint = c.goldHi, modifier = Modifier.size(22.dp))
                    }
                }
            }
        }
    }
}
