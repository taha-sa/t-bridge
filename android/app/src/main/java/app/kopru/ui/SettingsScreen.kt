package app.kopru.ui

import android.app.Activity
import app.kopru.Lang
import app.kopru.R
import androidx.compose.ui.res.stringResource
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.SystemUpdate
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.BuildConfig
import app.kopru.CrashLog
import app.kopru.UiState
import app.kopru.Updater

private fun Context.activity(): Activity? {
    var c: Context? = this
    while (c is ContextWrapper) {
        if (c is Activity) return c
        c = c.baseContext
    }
    return null
}

@Composable
fun SettingsScreen(@Suppress("UNUSED_PARAMETER") ui: UiState) {
    val c = tb
    val ctx = LocalContext.current
    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 18.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Text(stringResource(R.string.set_title), color = c.text, style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(top = 4.dp, bottom = 2.dp))
        ThemeCard()
        LanguageCard()
        UpdateCard()
        SecurityCard()
        CrashCard()
        About()
        SignatureFooter {
            val app = Intent(Intent.ACTION_VIEW, Uri.parse("https://instagram.com/_u/tdesk.studio")).setPackage("com.instagram.android")
            val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://instagram.com/tdesk.studio"))
            runCatching { ctx.startActivity(app) }.recoverCatching { ctx.startActivity(web) }
                .onFailure { Toast.makeText(ctx, ctx.getString(R.string.set_link_failed), Toast.LENGTH_SHORT).show() }
        }
    }
}

@Composable
private fun ThemeCard() {
    val c = tb
    val mode = ThemePref.mode.value
    TbCard {
        SectionTitle(stringResource(R.string.set_appearance))
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.set_theme), color = c.text, style = MaterialTheme.typography.titleSmall)
                Text(stringResource(R.string.set_theme_hint), color = c.muted, style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.width(10.dp))
            val shape = RoundedCornerShape(12.dp)
            Row(Modifier.clip(shape).background(c.s3).border(1.dp, c.line, shape).padding(3.dp)) {
                listOf("dark" to stringResource(R.string.set_theme_dark), "system" to stringResource(R.string.set_theme_system)).forEach { (id, label) ->
                    val sel = mode == id
                    Text(
                        label,
                        Modifier
                            .clip(RoundedCornerShape(9.dp))
                            .background(if (sel) c.goldSoft else Color.Transparent)
                            .clickable { ThemePref.set(id) }
                            .padding(horizontal = 13.dp, vertical = 7.dp),
                        color = if (sel) c.goldHi else c.muted,
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
            }
        }
    }
}

@Composable
private fun LanguageCard() {
    val c = tb
    val ctx = LocalContext.current
    var cur by remember { mutableStateOf(Lang.pref(ctx)) }
    val names = mapOf(Lang.SYSTEM to stringResource(R.string.set_lang_system), "tr" to "Türkçe", "en" to "English", "de" to "Deutsch")
    TbCard {
        SectionTitle(stringResource(R.string.set_language))
        Spacer(Modifier.height(4.dp))
        Text(stringResource(R.string.set_language_hint), color = c.muted, style = MaterialTheme.typography.bodySmall)
        Spacer(Modifier.height(10.dp))
        val shape = RoundedCornerShape(12.dp)
        Row(Modifier.fillMaxWidth().clip(shape).background(c.s3).border(1.dp, c.line, shape).padding(3.dp)) {
            Lang.CODES.forEach { code ->
                val sel = cur == code
                Text(
                    names.getValue(code),
                    Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(9.dp))
                        .background(if (sel) c.goldSoft else Color.Transparent)
                        .clickable {
                            if (cur != code) {
                                cur = code
                                Lang.set(ctx, code)
                                ctx.activity()?.recreate()
                            }
                        }
                        .padding(vertical = 8.dp),
                    color = if (sel) c.goldHi else c.muted,
                    style = MaterialTheme.typography.labelMedium,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    maxLines = 1,
                )
            }
        }
    }
}

@Composable
private fun UpdateCard() {
    val c = tb
    val ctx = LocalContext.current
    val st by Updater.state.collectAsStateWithLifecycle()
    var auto by remember { mutableStateOf(Updater.auto) }
    var tick by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { tick++ }
    @Suppress("UNUSED_EXPRESSION") tick
    val canInstall = Updater.canInstall()

    val text = when (val s = st) {
        Updater.State.Idle -> stringResource(R.string.set_upd_idle)
        Updater.State.Checking -> stringResource(R.string.set_upd_checking)
        Updater.State.Latest -> stringResource(R.string.set_upd_latest)
        is Updater.State.Downloading -> stringResource(R.string.set_upd_downloading, s.info.versionName, (s.progress * 100).toInt())
        is Updater.State.Ready -> stringResource(R.string.set_upd_ready, s.info.versionName)
        Updater.State.Installing -> stringResource(R.string.set_upd_installing)
        is Updater.State.Error -> stringResource(R.string.set_upd_error, s.message)
    }

    TbCard(gold = true) {
        SectionTitle(stringResource(R.string.set_update), color = c.gold)
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconTile(Icons.Outlined.SystemUpdate, size = 42.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("T-Bridge ${BuildConfig.VERSION_NAME}", color = c.text, style = MaterialTheme.typography.titleSmall)
                Text(text, color = if (st is Updater.State.Error) c.bad else c.muted, style = MaterialTheme.typography.bodySmall)
            }
        }
        (st as? Updater.State.Downloading)?.let {
            Spacer(Modifier.height(10.dp))
            GoldProgress(it.progress, active = true)
        }
        (st as? Updater.State.Ready)?.info?.notes?.takeIf { it.isNotBlank() }?.let {
            Spacer(Modifier.height(10.dp))
            val shape = RoundedCornerShape(12.dp)
            Box(Modifier.fillMaxWidth().clip(shape).background(c.s3).padding(12.dp)) {
                Text(it, color = c.text2, style = MaterialTheme.typography.bodySmall)
            }
        }
        Spacer(Modifier.height(14.dp))
        if (st is Updater.State.Ready) {
            GoldButton(stringResource(R.string.set_install_now), {
                ctx.activity()?.let { a ->
                    Updater.rememberReady()
                    Updater.install(a)
                }
            }, Modifier.fillMaxWidth(), icon = Icons.Outlined.SystemUpdate)
        } else {
            val busy = st is Updater.State.Checking || st is Updater.State.Downloading || st is Updater.State.Installing
            TbButton(stringResource(R.string.set_check_now), { Updater.check(manual = true) }, Modifier.fillMaxWidth(), icon = Icons.Outlined.Refresh, enabled = !busy)
        }
        Spacer(Modifier.height(6.dp))
        if (!canInstall) {
            CheckRow(stringResource(R.string.set_perm_title), stringResource(R.string.set_perm_desc), false) {
                ctx.activity()?.let(Updater::openInstallPermission)
            }
        }
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.set_auto), color = c.text, style = MaterialTheme.typography.titleSmall)
                Text(stringResource(R.string.set_auto_hint), color = c.muted, style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.width(10.dp))
            TbSwitch(auto) {
                auto = it
                Updater.auto = it
            }
        }
    }
}

@Composable
private fun SecurityCard() {
    val c = tb
    TbCard {
        SectionTitle(stringResource(R.string.set_security))
        Spacer(Modifier.height(12.dp))
        Text(stringResource(R.string.set_encrypted), color = c.text, style = MaterialTheme.typography.titleSmall)
        Text(stringResource(R.string.set_encrypted_hint), color = c.muted, style = MaterialTheme.typography.bodySmall)
    }
}

/** Yalnızca kayıtlı bir çökme varsa görünür. */
@Composable
private fun CrashCard() {
    val c = tb
    val ctx = LocalContext.current
    var log by remember { mutableStateOf(CrashLog.read()) }
    val text = log ?: return
    TbCard {
        SectionTitle(stringResource(R.string.set_crash))
        Spacer(Modifier.height(10.dp))
        Text(stringResource(R.string.set_crash_desc), color = c.text2, style = MaterialTheme.typography.bodySmall)
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            TbButton(stringResource(R.string.set_crash_copy), {
                ctx.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText(ctx.getString(R.string.set_crash_clip), text))
                Toast.makeText(ctx, ctx.getString(R.string.set_crash_copied), Toast.LENGTH_SHORT).show()
            }, Modifier.weight(1f), height = 44.dp)
            TbButton(stringResource(R.string.set_crash_delete), {
                CrashLog.clear()
                log = null
            }, Modifier.weight(1f), height = 44.dp)
        }
    }
}

@Composable
private fun About() {
    val c = tb
    Column(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Logo(46.dp)
        Spacer(Modifier.height(8.dp))
        Text("T-Bridge", color = c.text, style = MaterialTheme.typography.titleLarge)
        Text(stringResource(R.string.set_tagline, BuildConfig.VERSION_NAME), color = c.faint, style = MaterialTheme.typography.bodySmall)
    }
}
