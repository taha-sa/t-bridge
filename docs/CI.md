# CI ve sürümler

- `.github/workflows/ci.yml`: her push/PR. Windows işi (clippy, `kopru-core`/`tlink`/`desktop/src-tauri` testleri, `npm run check`) ve Android işi (`assembleDebug lintDebug`; JDK 21, NDK 27.2.12479018, `cargo-ndk`).
- `.github/workflows/release.yml`: `v*` etiketi. `environment: release` onayından sonra doğrular, sırları çalışma anında dosyaya yazar ve `node scripts/release.mjs <sürüm> "<not>" --ci` çalıştırır. Yayın `taha-sa/t-bridge-releases` deposuna `RELEASE_TOKEN` ile gider.

## Gerekli sırlar (Settings -> Secrets and variables -> Actions)

| Sır | Nasıl oluşturulur |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/tbridge.key` dosyasının tüm içeriği (yerelde: `Get-Content $HOME\.tauri\tbridge.key -Raw \| Set-Clipboard`, sonra yapıştır) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `~/.tauri/tbridge.key.pass` / `tbridge.pass` içindeki parola |
| `ANDROID_KEYSTORE_BASE64` | `certutil -encode android\kopru.keystore ks.b64`, ilk (`-----BEGIN`) ve son (`-----END`) satırları silinmiş içerik; ya da PowerShell: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("android\kopru.keystore")) \| Set-Clipboard` (tercih edilen). Sonra ara dosyayı silin. |
| `ANDROID_KEYSTORE_PASSWORD` | `android/keystore.properties` içindeki `storePassword` |
| `ANDROID_KEY_ALIAS` | aynı dosyadaki `keyAlias` |
| `ANDROID_KEY_PASSWORD` | aynı dosyadaki `keyPassword` |
| `RELEASE_TOKEN` | `taha-sa/t-bridge-releases` için Contents: write izinli ince ayarlı (fine-grained) token |

Bu sırları depo seviyesinde değil, tercihen `release` ortamının (environment) sırları olarak ekleyin; böylece yalnızca onaylı iş görür. `VIEWER_CERT_SHA256` zaten `android/gradle.properties` içinde (gizli değil). Anahtar dosyalarını asla sohbete/loga yazmayın.

## Ortam onayı

Settings -> Environments -> New environment -> adı tam olarak `release`. Required reviewers ekleyin, istenirse prevent self-review açın, Deployment branches and tags: `v*` etiketleriyle sınırlayın. Release işi bu onaya kadar bekler (diğer uygulamalarla aynı model, bkz. t-suite `docs/CI.md`).

## Sürüm çıkarmak

Etiket `version.txt` ile aynı kalıpta olmalı; geriye gidemez. Etiket mesajı (annotated tag) sürüm notu olur.

```powershell
# Kararlı: latest.json + android.json güncellenir, "latest" olarak yayınlanır
git tag -a v0.3.0 -m "Sürüm notu"; git push origin v0.3.0

# Beta: ön sürüm (prerelease); latest.json / android.json DEĞİŞMEZ
git tag -a v0.3.0-beta.1 -m "Beta notu"; git push origin v0.3.0-beta.1
```

Beta kanalı: masaüstü ve Android ayarlarında isteğe bağlıdır; varsayılan kararlı kanaldır. Beta yayını kendi `vX.Y.Z-beta.N` etiketiyle oluşturulur. `latest-beta.json` ve `android-beta.json`, `beta` adlı dönen ön sürümün varlıklarına da yüklenir. Kararlı `latest.json` / `android.json` korunur. Masaüstü sürümü SemVer beta son ekini taşır. Android versionCode = (major*10000 + minor*100 + patch)*1000 + sıra; beta sıra 1..998, aynı tabanın kararlı sürümü 999. Böylece beta güncellemeleri ve beta → kararlı geçiş artan sürüm koduyla yapılır. Daha eski bir kararlı tabana geri dönüş otomatik düşürme yapmaz. İmza denetimleri kanaldan bağımsız olarak korunur.

Yerel yayın değişmedi: `node scripts/release.mjs 0.3.0 "not" [--no-publish]`. `--ci` yalnızca CI içindir (doğrulamayı atlar, anahtarları ortam değişkenlerinden okur, git commit/tag yapmaz).
