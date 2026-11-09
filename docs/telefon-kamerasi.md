# Telefonu web kamerası olarak kullanma (Windows 11)

Telefon kamerası PC'de **"T-Bridge Kamera"** adlı sanal bir web kamerası olarak görünür (Zoom, Teams, Kamera uygulaması, tarayıcılar…).
Yetenek adı: `camera`. **Yalnızca Windows 11** (build ≥ 22000) — `MFCreateVirtualCamera` bu sürümde vardır; çekirdek sürücü yoktur.

## Windows 11 kapısı (kesin)
- Sürüm `RtlGetVersion` ile okunur (GetVersionEx uyumluluk katmanında yalan söyler). `kopru_core::camera::camera_allowed(is_windows, build)`:
  `Windows ve build ≥ 22000`. Birim testi: 19045 → kapalı, 21999 → kapalı, 22000 → açık, 26200 → açık, build okunamadı → kapalı, Windows değil → kapalı.
- Kapı kapalıysa PC **`camera` yeteneğini duyurmaz** (`proto::local_caps()` bayrağa bakar). Sonuç: telefonda Kamera bölümü
  "Bu özellik yalnızca Windows 11 için tasarlanmıştır." der; PC'de Kamera sayfası kilitli kart gösterir (kilit simgesi, soluk metin, düğme yok).
  Windows dışı derlemeler aynı yolu izler (`cfg!(windows)` yanlış → kapalı).

## Mimari
```
Telefon (Kotlin)                                PC
CameraX Preview ──Surface──▶ MediaCodec H.264    desktop (kopru-desktop)                     Frame Server (svchost, LocalService)
 (CBR, GOP 2 sn, düşük gecikme)                  ┌──────────────────────────────┐            ┌─────────────────────────────────┐
        │ Annex-B (+SPS/PPS her IDR)             │ Hub.push_frame → çözücü thread│            │ tbridge_vcam.dll                │
        └── QUIC tek yönlü akış (CAM_MAGIC) ───▶ │ MF H.264 çözücü → NV12        │  paylaşımlı│ Activate(IMFActivate)           │
              pts|flags|len çerçevesi            │ ayna? → Ring.write (NV12)     │ ─ bellek ─▶│  └ Source(IMFMediaSourceEx)     │
        ◀── CamStart / CamKeyframe / CamStop ─── │                               │ (Global\)  │     └ Stream(IMFMediaStream2)   │
        ──▶ CamState {active,w,h,fps,facing,..}  │ sanal kamera thread:          │            │ 30 fps örnek; 1 sn taze kare    │
                                                 │ MFCreateVirtualCamera(Session,│            │ yoksa "Telefon bekleniyor" kare │
                                                 │ CurrentUser)                  │            └──────────────▲──────────────────┘
                                                 └──────────────────────────────┘                           │ Zoom / Teams / Kamera
```
- **Protokol** (`core/src/proto.rs`): `CamStart{w,h,fps,bitrate_kbps}` (PC→tel), `CamStop`, `CamKeyframe`, `CamState{active,w,h,fps,facing,mirror}` (tel→PC).
  Video akışı ekran paylaşımıyla aynı çerçeveleme ve geri basınç kapısını kullanır (`screen::Gate`, 150 ms), yalnızca farklı sihirli sayı (`CAM_MAGIC`) ve yön.
  İki taraf `camera` yeteneğini duyurmadıkça hiçbir `Cam*` mesajı gönderilmez; eski eşler etkilenmez.
- **Halka** (`camera/ring`, `kopru-camring`): başlık + 3 yuva, yuva başına seqlock; yazıcı = uygulama, okuyucu = DLL. 9 birim testi: yazıcı/okuyucu, sarma,
  çözünürlük değişimi, bayatlık, yırtık okuma reddi, eşzamanlı yazıcı/okuyucu, ayna.
- **Bölümü DLL yaratır** (`Global\TBridgeCameraRing`): `Global\` ad alanında bölüm *yaratmak* SeCreateGlobalPrivilege ister (LocalService'te var, standart kullanıcıda yok);
  *açmak* istemez. DACL (SDDL): SYSTEM + LocalService tam, etkileşimli kullanıcı okuma/yazma; diğerleri yok.
  Sonuç: kimse kamerayı açmadıkça bölüm yoktur → uygulama gelen kareleri **çözmeden atar** (CPU harcanmaz); bölüm belirince anahtar kare ister.
- **Biçim**: DLL NV12 1280×720@30 ve 1920×1080@30 sunar; istemcinin seçtiği biçimi halkaya `want` olarak yazar, uygulama telefondan bu boyutu ister
  (`CamStart`). Boyut uyuşmazsa DLL en yakın komşu ölçekler. YUY2 eklenmedi (NV12 çoğu istemci için yeterli; Frame Server dönüştürür).
- **Yer tutucu**: 1 sn taze kare yoksa koyu `#0a0a0b` zemin üstüne altın `#e3a92b` 5×7 bitmap yazı: "T-Bridge · Telefon bekleniyor" (pişirilmiş, yazı tipi yok).

## Araç zinciri (neden Rust/windows-rs)
Kaynak, Microsoft'un [Windows-Camera VirtualCamera örneği](https://github.com/microsoft/Windows-Camera) (MIT) ve smourier/VCamSample (MIT) mimarisine dayanır
(IMFActivate → IMFMediaSourceEx → IMFMediaStream2, `MF_DEVICESTREAM_*` öznitelikleri); **kod özgündür, örneklerden kod kopyalanmadı**.
Rust + `windows` 0.62 seçildi: depo zaten Rust çalışma alanı; MSVC bağlayıcısı kurulu ve `cargo build` ile yeniden üretilebilir (CMake/MSBuild ve C++/WinRT NuGet
bağımlılığı yok); `#[implement]` COM nesnelerini güvenli üretir. Çıktı: `target/release/tbridge_vcam.dll` (~165 KB), dışa aktarımlar `DllGetClassObject`, `DllCanUnloadNow`.

## Kayıt (bir kerelik yönetici izni)
1. Uygulama → Kamera → **"Kamerayı etkinleştir"**.
2. Kayıt yoksa uygulama kendini `--camera-register "<dll>"` ile **UAC (runas)** üzerinden yeniden başlatır; yükseltilmiş süreç DLL'i
   `%ProgramFiles%\T-Bridge\camera\` altına kopyalar (LocalService okuyabilsin) ve HKLM'ye `CLSID\{7B3F1C52-4D2E-4C71-9A0B-5E2D8F6A1C34}\InprocServer32` (+ `ThreadingModel=Both`) yazar.
3. Normal süreç `MFCreateVirtualCamera(SoftwareCameraSource, Session, CurrentUser, "T-Bridge Kamera", CLSID)` + `Start` çağırır.
   Ömür *Session*: T-Bridge kapanınca (veya "Durdur") kamera kalkar. `camera_enabled` ayarı açıksa her açılışta otomatik başlatılır (kayıt varsa).
4. **"Kamerayı kaldır"**: `--camera-unregister` (UAC) HKLM kaydını ve DLL'i siler.
NSIS: `installer-hooks.nsh` kaldırma sırasında kayıt varsa yalnızca bu adım için UAC ister (kurulum kullanıcı başına kalır).
DLL dağıtımı: `tbridge_vcam.dll` uygulama exe'sinin yanında (veya `camera\`) bulunmalıdır; geliştirmede `cargo build -p tbridge-vcam --release` çıktısını exe klasörüne kopyalayın.
(Tauri `bundle.resources` girdisi eklenmedi: dosya yokken derleme kırılmasın; release betiğine eklenmelidir.)

## Telefon (Kotlin)
- `CameraService` (ön plan hizmeti, `foregroundServiceType="camera"`, Android 14): CameraX `Preview` çıktısı doğrudan `MediaCodec` giriş yüzeyine; CBR, GOP 2 sn,
  `KEY_LOW_LATENCY` (API 30+), Baseline profil; SPS/PPS her IDR'ye eklenir; geri basınçta bit hızı düşer, kare atılırsa anahtar kare istenir.
  Bildirim: "T-Bridge kamera olarak kullanılıyor". Ekran kapalıyken yayın hizmet sayesinde sürer.
- Kontroller (Kumanda → Kamera): ön/arka, 720p/1080p, ayna, fener, pozlama ±, yakınlık (lens zoom), "Yayını durdur". İzin akışı gerekçe metniyle.
- Ayna PC'de uygulanır (NV12 yatay çevirme). Görüntü sensör yönündedir: telefon yatay tutulunca dik görünür (döndürme seçeneği yok).

## Sorun giderme
| Belirti | Çözüm |
|---|---|
| "T-Bridge Kamera" listede yok | T-Bridge'i kapat/aç, kamerayı kullanan uygulamayı yeniden başlat; Kamera sayfasında durum "Hazır/Yayında" mı bak |
| Siyah/"Telefon bekleniyor" | Telefonda yayın kapalı ya da bağlantı kopuk; Wi-Fi aynı ağ mı |
| Etkinleştirme reddedildi | UAC onayı gerekir; reddedilirse tekrar dene |
| Windows 10 | Özellik tasarım gereği kapalı |
| Güncelleme sonrası kamera yok | "Kamerayı kaldır" → "Kamerayı etkinleştir" (DLL yeniden kaydedilir) |

## Doğrulananlar / doğrulanmayanlar
- Doğrulandı (derleme + birim testi): kapı mantığı, halka (9 test), yer tutucu/ölçekleme/ayna, protokol, DLL ve masaüstü derlemesi, Svelte kontrolü, Android derlemesi.
- **Doğrulanmadı** (bu oturumda yönetici izni/kayıt yok, DLL kaydedilmedi): Frame Server'ın DLL'i yükleyip yüklemediği, `IMFActivate` ile etkinleştirme varsayımı,
  öznitelik kümesinin Zoom/Teams tarafından kabulü, canlı QUIC akışı, MF H.264 çözücü çıktı yerleşimi (stride/hizalı yükseklik varsayımı), telefonda CameraX/MediaCodec yolu.
