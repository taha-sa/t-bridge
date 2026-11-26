import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val repoRoot: File = rootDir.parentFile
val uniffiOut = layout.buildDirectory.dir("generated/uniffi/kotlin")
val jniOut = layout.buildDirectory.dir("generated/jniLibs")

// İmzalama: android/keystore.properties varsa release imzalı olur (yoksa debug anahtarı).
// Sürüm tek kaynaktan: repo kökündeki version.txt (x.y.z → versionCode x*10000+y*100+z).
val appVersion: String = (findProperty("releaseVersion") as String?) ?: repoRoot.resolve("version.txt").readText().trim()
val versionMatch = Regex("""^(\d+)\.(\d+)\.(\d+)(?:-beta\.?(\d+))?$""").matchEntire(appVersion)
    ?: error("Invalid releaseVersion")
val appVersionCode: Int = versionMatch.groupValues.let { parts ->
    require(parts[2].toInt() in 0..99 && parts[3].toInt() in 0..99)
    val base = parts[1].toInt() * 10000 + parts[2].toInt() * 100 + parts[3].toInt()
    val revision = parts[4].takeIf { it.isNotEmpty() }?.toInt() ?: 999
    require(revision in 1..999 && (parts[4].isEmpty() || revision < 999))
    Math.addExact(Math.multiplyExact(base, 1000), revision).also { require(it in 1..2100000000) }
}

val keyProps = Properties().apply {
    val f = rootProject.file("keystore.properties")
    if (f.exists()) f.inputStream().use { load(it) }
}

android {
    namespace = "app.kopru"
    compileSdk = 37

    defaultConfig {
        applicationId = "app.kopru"
        minSdk = 29
        targetSdk = 35
        versionCode = appVersionCode
        versionName = appVersion
        ndk { abiFilters += listOf("arm64-v8a") }
        // T-Desk Viewer yayın imzasının SHA-256 parmak izi (virgülle birden çok; iki nokta/boşluk yok sayılır). Kaynak: gradle özelliği
        // `viewerCertSha256` (gradle.properties / -P) veya ortam değişkeni VIEWER_CERT_SHA256. Boşsa: debug esnek, release Viewer'ı REDDEDER.
        val viewerCert = ((findProperty("viewerCertSha256") as String?) ?: System.getenv("VIEWER_CERT_SHA256") ?: "").trim()
        buildConfigField("String", "VIEWER_CERT_SHA256", "\"" + viewerCert.replace("\"", "") + "\"")
    }

    signingConfigs {
        create("release") {
            val sf = keyProps.getProperty("storeFile")
            if (!sf.isNullOrBlank()) {
                storeFile = rootProject.file(sf)
                storePassword = keyProps.getProperty("storePassword")
                keyAlias = keyProps.getProperty("keyAlias")
                keyPassword = keyProps.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (!keyProps.getProperty("storeFile").isNullOrBlank()) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    // Eksik çeviri derlemeyi bozar (EN varsayılan; TR ve DE tam olmalı).
    lint {
        error += listOf("MissingTranslation", "ExtraTranslation")
        // Geri çağrılarda çözülen metinler: dil değişince etkinlik yeniden oluşturulur.
        disable += "LocalContextGetResourceValueCall"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets {
        getByName("main") {
            java.srcDir(uniffiOut.get().asFile)
            kotlin.srcDir(uniffiOut.get().asFile)
            jniLibs.srcDir(jniOut.get().asFile)
        }
    }

    packaging {
        jniLibs { useLegacyPackaging = false }
    }
}

// --- Rust çekirdeği: cargo-ndk ile derle, UniFFI ile Kotlin bağlamalarını üret ---
val cargoBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "kopru-core'u arm64-v8a için derler"
    workingDir = repoRoot
    inputs.dir(File(repoRoot, "core/src"))
    inputs.file(File(repoRoot, "core/Cargo.toml"))
    outputs.dir(jniOut)
    commandLine(
        "cargo", "ndk", "-t", "arm64-v8a", "--platform", "29",
        "-o", jniOut.get().asFile.absolutePath,
        "build", "--release", "-p", "kopru-core", "--lib",
    )
}

// Bağlamalar ana makine (debug) kütüphanesinden üretilir: release'teki strip, UniFFI meta verisini siler.
val hostLib: File = File(repoRoot, "target/debug/" + when {
    System.getProperty("os.name").lowercase().contains("win") -> "kopru_core.dll"
    System.getProperty("os.name").lowercase().contains("mac") -> "libkopru_core.dylib"
    else -> "libkopru_core.so"
})

val cargoHostBuild by tasks.registering(Exec::class) {
    group = "rust"
    workingDir = repoRoot
    inputs.dir(File(repoRoot, "core/src"))
    inputs.file(File(repoRoot, "core/Cargo.toml"))
    outputs.file(hostLib)
    commandLine("cargo", "build", "-q", "-p", "kopru-core", "--lib")
}

val uniffiBindgen by tasks.registering(Exec::class) {
    group = "rust"
    description = "UniFFI Kotlin bağlamalarını üretir"
    dependsOn(cargoBuild, cargoHostBuild)
    workingDir = repoRoot
    inputs.file(hostLib)
    outputs.dir(uniffiOut)
    commandLine(
        "cargo", "run", "-q", "-p", "kopru-core", "--features", "bindgen", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", hostLib.absolutePath, "--language", "kotlin", "--no-format",
        "--out-dir", uniffiOut.get().asFile.absolutePath,
    )
}

tasks.named("preBuild") { dependsOn(uniffiBindgen) }

dependencies {
    testImplementation("junit:junit:4.13.2")
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    // QR: Play Hizmetleri modülüne bağlı kalmadan uygulama içi kamera okuyucu.
    val camerax = "1.6.2"
    implementation("androidx.camera:camera-camera2:$camerax")
    implementation("androidx.camera:camera-lifecycle:$camerax")
    implementation("androidx.camera:camera-view:$camerax")
    implementation("com.google.zxing:core:3.5.4")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
}
