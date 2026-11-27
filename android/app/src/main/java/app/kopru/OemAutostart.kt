package app.kopru

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings

/**
 * Üreticiye özgü arka plan / otomatik başlatma ayar yönlendiricisi.
 * Saf Android cihazlarda (Pixel, Nothing, Motorola vb.) ek OEM adımı gerekmez -> null döner;
 * agresif arka plan öldürme yapan üreticilerde ilgili ayar sayfasını açar, açamazsa uygulama bilgisine düşer.
 */
object OemAutostart {
    enum class Kind {
        XIAOMI,
        SAMSUNG,
        HUAWEI,
        OPPO_FAMILY,
        VIVO,
        ASUS,
        TRANSSION,
        MEIZU,
    }

    data class Profile(
        val kind: Kind,
        val brandLabel: String,
        val components: List<Pair<String, String>>,
    )

    fun detect(
        manufacturer: String = Build.MANUFACTURER.orEmpty(),
        brand: String = Build.BRAND.orEmpty(),
    ): Profile? {
        val m = manufacturer.trim().lowercase()
        val b = brand.trim().lowercase()
        fun any(vararg keys: String): Boolean = keys.any { k -> m == k || b == k || m.contains(k) || b.contains(k) }

        return when {
            any("xiaomi", "redmi", "poco") -> Profile(
                kind = Kind.XIAOMI,
                brandLabel = "HyperOS / MIUI",
                components = listOf(
                    "com.miui.securitycenter" to "com.miui.permcenter.autostart.AutoStartManagementActivity",
                    "com.miui.securitycenter" to "com.miui.powercenter.PowerSettings",
                ),
            )
            any("samsung") -> Profile(
                kind = Kind.SAMSUNG,
                brandLabel = "Samsung",
                components = listOf(
                    "com.samsung.android.lool" to "com.samsung.android.sm.battery.ui.BatteryActivity",
                    "com.samsung.android.sm_cn" to "com.samsung.android.sm.ui.ram.AutoRunActivity",
                    "com.samsung.android.sm" to "com.samsung.android.sm.battery.ui.BatteryActivity",
                ),
            )
            any("honor") -> Profile(
                kind = Kind.HUAWEI,
                brandLabel = "Honor",
                components = huaweiComponents,
            )
            any("huawei") -> Profile(
                kind = Kind.HUAWEI,
                brandLabel = "Huawei",
                components = huaweiComponents,
            )
            any("oneplus") -> Profile(
                kind = Kind.OPPO_FAMILY,
                brandLabel = "OnePlus",
                components = listOf(
                    "com.oneplus.security" to "com.oneplus.security.chainlaunch.view.ChainLaunchAppListActivity",
                ) + oppoComponents,
            )
            any("realme") -> Profile(
                kind = Kind.OPPO_FAMILY,
                brandLabel = "Realme",
                components = oppoComponents,
            )
            any("oppo") -> Profile(
                kind = Kind.OPPO_FAMILY,
                brandLabel = "OPPO",
                components = oppoComponents,
            )
            any("vivo", "iqoo") -> Profile(
                kind = Kind.VIVO,
                brandLabel = if (b == "iqoo" || m == "iqoo") "iQOO" else "Vivo",
                components = listOf(
                    "com.iqoo.secure" to "com.iqoo.secure.ui.phoneoptimize.AddWhiteListActivity",
                    "com.vivo.permissionmanager" to "com.vivo.permissionmanager.activity.BgStartUpManagerActivity",
                    "com.iqoo.secure" to "com.iqoo.secure.ui.phoneoptimize.BgStartUpManager",
                ),
            )
            any("asus") -> Profile(
                kind = Kind.ASUS,
                brandLabel = "ASUS",
                components = listOf(
                    "com.asus.mobilemanager" to "com.asus.mobilemanager.autostart.AutoStartActivity",
                    "com.asus.mobilemanager" to "com.asus.mobilemanager.MainActivity",
                ),
            )
            any("tecno", "infinix", "itel") -> Profile(
                kind = Kind.TRANSSION,
                brandLabel = when {
                    b == "infinix" || m == "infinix" -> "Infinix"
                    b == "itel" || m == "itel" -> "Itel"
                    else -> "Tecno"
                },
                components = listOf(
                    "com.transsion.phonemaster" to "com.cyin.himgr.autostart.AutoStartActivity",
                ),
            )
            any("meizu") -> Profile(
                kind = Kind.MEIZU,
                brandLabel = "Meizu",
                components = listOf(
                    "com.meizu.safe" to "com.meizu.safe.permission.SmartBGActivity",
                ),
            )
            else -> null
        }
    }

    private val huaweiComponents = listOf(
        "com.huawei.systemmanager" to "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity",
        "com.huawei.systemmanager" to "com.huawei.systemmanager.appcontrol.activity.StartupAppControlActivity",
        "com.huawei.systemmanager" to "com.huawei.systemmanager.optimize.process.ProtectActivity",
    )

    private val oppoComponents = listOf(
        "com.coloros.safecenter" to "com.coloros.safecenter.permission.startup.StartupAppListActivity",
        "com.oplus.battery" to "com.oplus.powermanager.fuelgaue.PowerUsageModelActivity",
        "com.coloros.safecenter" to "com.coloros.safecenter.startupapp.StartupAppListActivity",
        "com.oppo.safe" to "com.oppo.safe.permission.startup.StartupAppListActivity",
    )

    fun openSettings(ctx: Context, profile: Profile) {
        val intents = profile.components.map { (pkg, cls) ->
            Intent().setComponent(ComponentName(pkg, cls))
        } + Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}"))
        startFirst(ctx, intents)
    }

    @SuppressLint("BatteryLife")
    fun openBatteryUnrestricted(ctx: Context) {
        val intents = listOf(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${ctx.packageName}")),
            Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS),
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:${ctx.packageName}")),
        )
        startFirst(ctx, intents)
    }

    private fun startFirst(ctx: Context, intents: List<Intent>) {
        for (i in intents) {
            try {
                ctx.startActivity(i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                return
            } catch (_: ActivityNotFoundException) {
            } catch (_: SecurityException) {
            }
        }
    }
}
