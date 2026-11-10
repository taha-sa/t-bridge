package app.kopru

import android.app.LocaleManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/**
 * Uygulama dili: "system" (varsayılan) | "tr" | "en" | "de".
 * Android 13+: sistemin uygulama başına dil ayarı (LocaleManager); daha eski sürümlerde kayıtlı tercih ile bağlam sarılır.
 */
object Lang {
    const val SYSTEM = "system"
    val CODES = listOf(SYSTEM, "tr", "en", "de")

    private fun prefs(ctx: Context) = ctx.getSharedPreferences("lang", Context.MODE_PRIVATE)

    fun pref(ctx: Context): String {
        // Android 13+ sistem ayarından da değiştirilebilir: önce oradan oku.
        if (Build.VERSION.SDK_INT >= 33) {
            val l = ctx.getSystemService(LocaleManager::class.java)?.applicationLocales
            if (l != null) return if (l.isEmpty) SYSTEM else l[0].language.takeIf { it in CODES } ?: SYSTEM
        }
        return prefs(ctx).getString("pref", SYSTEM)?.takeIf { it in CODES } ?: SYSTEM
    }

    /** Tercihi kaydeder ve (13+) uygular. Eski sürümlerde etkinliğin yeniden oluşturulması gerekir. */
    fun set(ctx: Context, code: String) {
        val c = if (code in CODES) code else SYSTEM
        prefs(ctx).edit().putString("pref", c).apply()
        if (Build.VERSION.SDK_INT >= 33) {
            ctx.getSystemService(LocaleManager::class.java)?.applicationLocales =
                if (c == SYSTEM) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(c)
        }
    }

    /** Android 12 ve altı: tercihi bağlama uygula. Etkinliklerin ve Application'ın attachBaseContext'inde çağrılır. */
    fun wrap(base: Context): Context {
        if (Build.VERSION.SDK_INT >= 33) return base
        val code = prefs(base).getString("pref", SYSTEM) ?: SYSTEM
        if (code == SYSTEM || code !in CODES) return base
        val cfg = Configuration(base.resources.configuration)
        val loc = Locale.forLanguageTag(code)
        Locale.setDefault(loc)
        cfg.setLocale(loc)
        return base.createConfigurationContext(cfg)
    }
}
