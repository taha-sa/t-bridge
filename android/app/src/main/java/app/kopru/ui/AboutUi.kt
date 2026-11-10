package app.kopru.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kopru.CrashLog
import app.kopru.R
import app.kopru.ReportPreview
import app.kopru.Suite
import app.kopru.ViewerLink
import org.json.JSONArray

private enum class Legal { PRIVACY, LICENSE, DISCLAIMER, NOTICES }

private fun open(ctx: Context, url: String) {
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

private fun nameOf(a: InviteApp) = when (a) {
    InviteApp.DOWNLOADER -> "T-Downloader"
    InviteApp.RAMCASH -> "T-RamCash"
    InviteApp.DESKTREE -> "T-DeskTree"
    InviteApp.VIEWER -> "T-Desk Viewer"
}

/** Hakkında: sürüm, T-Suite ailesi (kurulu / edinin), hata raporu ve yasal metinler. */
@Composable
fun AboutCard() {
    val c = tb
    val ctx = LocalContext.current
    val suite by Suite.ui.collectAsStateWithLifecycle()
    val viewer by ViewerLink.state.collectAsStateWithLifecycle()
    var legal by remember { mutableStateOf<Legal?>(null) }
    var report by remember { mutableStateOf<ReportPreview?>(null) }
    TbCard {
        SectionTitle(stringResource(R.string.about_title))
        Spacer(Modifier.size(12.dp))
        Text("T-Bridge", color = c.text, style = MaterialTheme.typography.titleSmall)
        Text(
            stringResource(R.string.about_version, app.kopru.BuildConfig.VERSION_NAME) + " · " + stringResource(R.string.about_family),
            color = c.muted, style = MaterialTheme.typography.bodySmall,
        )
        Spacer(Modifier.size(10.dp))
        for (a in InviteApp.entries) {
            val isViewer = a == InviteApp.VIEWER
            val installed = if (isViewer) viewer.installed else suite.apps.any { it.id == a.id }
            Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Image(painterResource(a.icon), null, Modifier.size(32.dp).clip(RoundedCornerShape(8.dp)))
                Column(Modifier.weight(1f).padding(start = 12.dp)) {
                    Text(nameOf(a), color = c.text, style = MaterialTheme.typography.titleSmall)
                    if (isViewer) Text(stringResource(R.string.about_phone_app), color = c.faint, style = MaterialTheme.typography.bodySmall)
                }
                if (installed) {
                    Text(stringResource(R.string.about_installed), color = c.ok, style = MaterialTheme.typography.labelLarge)
                } else {
                    Box(
                        Modifier.heightIn(min = 48.dp).clip(RoundedCornerShape(8.dp)).clickable { open(ctx, a.url) }.padding(horizontal = 8.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            stringResource(
                                when (a) {
                                    InviteApp.DOWNLOADER -> R.string.about_get_dl
                                    InviteApp.RAMCASH -> R.string.about_get_ram
                                    InviteApp.DESKTREE -> R.string.about_get_tree
                                    InviteApp.VIEWER -> R.string.about_get_viewer
                                },
                            ),
                            color = c.gold, style = MaterialTheme.typography.labelLarge,
                        )
                    }
                }
            }
        }
        Spacer(Modifier.size(8.dp))
        TbButton(stringResource(R.string.about_report_bug), { report = CrashLog.manual() }, Modifier.fillMaxWidth(), height = 44.dp)
        Spacer(Modifier.size(14.dp))
        SectionTitle(stringResource(R.string.about_legal))
        Spacer(Modifier.size(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            TbButton(stringResource(R.string.about_privacy), { legal = Legal.PRIVACY }, Modifier.weight(1f), height = 44.dp)
            TbButton(stringResource(R.string.about_license), { legal = Legal.LICENSE }, Modifier.weight(1f), height = 44.dp)
        }
        Spacer(Modifier.size(10.dp))
        TbButton(stringResource(R.string.about_disclaimer), { legal = Legal.DISCLAIMER }, Modifier.fillMaxWidth(), height = 44.dp)
        Spacer(Modifier.size(10.dp))
        TbButton(stringResource(R.string.about_notices), { legal = Legal.NOTICES }, Modifier.fillMaxWidth(), height = 44.dp)
    }
    legal?.let { LegalDialog(it) { legal = null } }
    report?.let { ReportDialog(it, manual = true) { report = null } }
}

/** "Hata raporlarını otomatik hazırla" anahtarı. */
@Composable
fun DiagnosticsCard() {
    val c = tb
    var auto by remember { mutableStateOf(CrashLog.auto) }
    TbCard {
        SectionTitle(stringResource(R.string.set_diagnostics))
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.crash_auto), color = c.text, style = MaterialTheme.typography.titleSmall)
                Text(stringResource(R.string.crash_auto_hint), color = c.muted, style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.width(10.dp))
            TbSwitch(auto) {
                auto = it
                CrashLog.auto = it
            }
        }
    }
}

/** Önceki oturumdan kalan çökme raporu varsa açılışta gösterilir. */
@Composable
fun CrashPrompt() {
    var pending by remember { mutableStateOf<ReportPreview?>(null) }
    LaunchedEffect(Unit) { pending = CrashLog.pending() }
    pending?.let {
        ReportDialog(it, manual = false) {
            CrashLog.dismiss()
            pending = null
        }
    }
}

@Composable
private fun ReportDialog(p: ReportPreview, manual: Boolean, onClose: () -> Unit) {
    val c = tb
    val ctx = LocalContext.current
    Dialog(onDismissRequest = onClose) {
        Column(
            Modifier.clip(RoundedCornerShape(16.dp)).background(c.s1).border(1.dp, c.line, RoundedCornerShape(16.dp)).padding(18.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text(stringResource(if (manual) R.string.crash_manual_title else R.string.crash_title), color = c.text, style = MaterialTheme.typography.titleMedium)
            Text(stringResource(if (manual) R.string.crash_body_manual else R.string.crash_body), color = c.muted, style = MaterialTheme.typography.bodySmall)
            Box(Modifier.heightIn(max = 260.dp).clip(RoundedCornerShape(8.dp)).background(c.bg).padding(10.dp)) {
                Text(p.text, color = c.text2, style = MaterialTheme.typography.bodySmall, modifier = Modifier.verticalScroll(rememberScrollState()))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TbButton(stringResource(R.string.crash_close), onClose, Modifier.weight(1f), height = 44.dp)
                TbButton(stringResource(R.string.crash_copy), {
                    ctx.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("T-Bridge", p.text))
                    Toast.makeText(ctx, ctx.getString(R.string.crash_copied), Toast.LENGTH_SHORT).show()
                }, Modifier.weight(1f), height = 44.dp)
            }
            TbButton(stringResource(R.string.crash_send), {
                open(ctx, p.url)
                onClose()
            }, Modifier.fillMaxWidth(), height = 48.dp)
        }
    }
}

private fun lang(ctx: Context): String = ctx.resources.configuration.locales[0].language.let { if (it == "tr" || it == "de") it else "en" }

private fun readAsset(ctx: Context, name: String): String = runCatching { ctx.assets.open("legal/$name").bufferedReader().use { it.readText() } }.getOrDefault("")

/** Basit Markdown: ### başlık, **kalın**, [metin](url) → "metin (url)". */
private fun mdText(src: String): AnnotatedString = buildAnnotatedString {
    for (raw in src.replace("\r\n", "\n").lines()) {
        val line = raw.trimEnd().replace(Regex("""\[([^\]]+)]\((https://[^)\s]+)\)""")) { "${it.groupValues[1]} (${it.groupValues[2]})" }
        if (line.trim() == "---") continue
        val h = Regex("""^#{1,4}\s+(.*)$""").find(line)
        if (h != null) {
            withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(h.groupValues[1]) }
            append("\n")
            continue
        }
        val bullet = Regex("""^\s*[*-]\s+(.*)$""").find(line)
        val body = if (bullet != null) "• " + bullet.groupValues[1] else line.trim()
        var i = 0
        for (m in Regex("""\*\*(.+?)\*\*""").findAll(body)) {
            append(body.substring(i, m.range.first))
            withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(m.groupValues[1]) }
            i = m.range.last + 1
        }
        append(body.substring(i))
        append("\n")
    }
}

@Composable
private fun LegalDialog(kind: Legal, onClose: () -> Unit) {
    val c = tb
    val ctx = LocalContext.current
    val title = stringResource(
        when (kind) {
            Legal.PRIVACY -> R.string.about_privacy
            Legal.LICENSE -> R.string.about_license
            Legal.DISCLAIMER -> R.string.about_disclaimer
            Legal.NOTICES -> R.string.about_notices
        },
    )
    Dialog(onDismissRequest = onClose) {
        Column(Modifier.clip(RoundedCornerShape(16.dp)).background(c.s1).border(1.dp, c.line, RoundedCornerShape(16.dp)).padding(18.dp)) {
            Text(title, color = c.text, style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.size(10.dp))
            Box(Modifier.weight(1f, fill = false).heightIn(max = 460.dp)) {
                when (kind) {
                    Legal.NOTICES -> {
                        val rows = remember { runCatching { JSONArray(readAsset(ctx, "notices.json")) }.getOrDefault(JSONArray()) }
                        Column {
                            Text(stringResource(R.string.about_notices_count, rows.length()), color = c.muted, style = MaterialTheme.typography.bodySmall)
                            Spacer(Modifier.size(6.dp))
                            LazyColumn {
                                items(rows.length()) { i ->
                                    val r = rows.getJSONArray(i)
                                    Column(Modifier.padding(vertical = 4.dp)) {
                                        Text("${r.getString(0)} ${r.getString(1)}", color = c.text2, style = MaterialTheme.typography.bodySmall)
                                        Text(r.getString(2), color = c.faint, style = MaterialTheme.typography.labelSmall)
                                    }
                                }
                            }
                        }
                    }
                    else -> {
                        val text = remember(kind) {
                            when (kind) {
                                Legal.LICENSE -> AnnotatedString(readAsset(ctx, "LICENSE.txt"))
                                Legal.PRIVACY -> mdText(readAsset(ctx, "privacy.${lang(ctx)}.md"))
                                else -> mdText(readAsset(ctx, "disclaimer.${lang(ctx)}.md"))
                            }
                        }
                        Text(text, color = c.text2, style = MaterialTheme.typography.bodySmall, modifier = Modifier.verticalScroll(rememberScrollState()))
                    }
                }
            }
            Spacer(Modifier.size(12.dp))
            TbButton(stringResource(R.string.crash_close), onClose, Modifier.fillMaxWidth(), height = 44.dp)
        }
    }
}
