package app.kopru

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.ui.res.stringResource
import app.kopru.ui.InviteApp
import app.kopru.ui.InviteDialog
import app.kopru.ui.TBridgeTheme
import app.kopru.ui.openInviteUrl

/**
 * "Telefonda aç": T-Desk Viewer kurulu değilse dosya sistem seçicisine gitmeden önce bir kez davet gösterilir.
 * × yalnızca bu daveti kapatır; "Başka uygulamayla aç" dosyayı sistem seçicisinde açar.
 */
class ViewerInviteActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: Context) = super.attachBaseContext(Lang.wrap(newBase))

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val uri: Uri? = intent.data
        val name = intent.getStringExtra(EXTRA_NAME) ?: ""
        val mime = intent.getStringExtra(EXTRA_MIME) ?: "*/*"
        setContent {
            TBridgeTheme {
                InviteDialog(
                    app = InviteApp.VIEWER,
                    title = stringResource(R.string.inv_open_title),
                    body = stringResource(R.string.inv_open_body),
                    cta = stringResource(R.string.inv_viewer_cta),
                    onCta = { openInviteUrl(this, InviteApp.VIEWER); finish() },
                    onClose = { finish() },
                    secondary = stringResource(R.string.inv_open_other),
                    onSecondary = {
                        if (uri != null) {
                            val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime)
                                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
                            view.clipData = android.content.ClipData.newRawUri(name, uri)
                            runCatching { startActivity(Intent.createChooser(view, getString(R.string.viewer_chooser))) }
                        }
                        finish()
                    },
                )
            }
        }
    }

    companion object {
        private const val EXTRA_NAME = "name"
        private const val EXTRA_MIME = "mime"

        fun intent(ctx: Context, uri: Uri, name: String, mime: String): Intent =
            Intent(ctx, ViewerInviteActivity::class.java)
                .setData(uri)
                .putExtra(EXTRA_NAME, name)
                .putExtra(EXTRA_MIME, mime)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION)
    }
}
