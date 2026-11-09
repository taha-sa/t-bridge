# Uzaktan ekran görüntüleme (PC → telefon)

Telefon, PC ekranını canlı izler; dokunma hareketleri PC'de fare/klavye girdisine çevrilir. Yetenek adı: `screen-share`
(Hello/Welcome `caps`). İki taraf da duyurmadıkça hiçbir `Screen*` mesajı veya video akışı kullanılmaz (eski eşler etkilenmez).

## Mimari

```
PC (desktop/src-tauri)                                        Telefon (android)
┌───────────────────────────────────────────────┐             ┌──────────────────────────────┐
│ Yakalama  WGC (imleç dahil, çerçevesiz Win11)   │             │ Screen.kt                    │
│           └ yedek: DXGI Desktop Duplication     │             │  MediaCodec H.264 → SurfaceView│
│ Dönüştür  D3D11 Video İşlemcisi: BGRA→NV12 +    │  QUIC       │  KEY_LOW_LATENCY, render=true │
│           ölçekleme (GPU, en-boy korunur)        │ uni akış    │  dolu/geç kare at + IDR iste  │
│ Kodla     Media Foundation H.264 MFT            │ ─────────▶  │ ScreenView.kt (hareketler)    │
│           donanım→sıfır kopya, yoksa yazılım      │ video       │  tık/sürükle/kaydır/çimdik    │
│ Geri basınç Gate (kuyruk >150 ms → P at, IDR)    │ ◀───────── │ RemoteScreen (açma, tam ekran)│
│ Uyarlanabilir bit hızı (adapt_bitrate)           │ kontrol     └──────────────────────────────┘
└───────────────────────────────────────────────┘ ScreenStart/Stop/Keyframe/Config/Info/Pointer
```

### Protokol (core/src/proto.rs, screen.rs)
| Mesaj | Yön | İçerik |
|---|---|---|
| `ScreenStart` | tel→PC | monitor, max_w, max_h, fps, bitrate_kbps |
| `ScreenStop` / `ScreenKeyframe` | tel→PC | – |
| `ScreenConfig` | tel→PC | canlı kalite değişimi (boru hattı yeni parametrelerle yeniden kurulur, ilk kare IDR) |
| `ScreenInfo` | PC→tel | monitör listesi, seçili, aktif mi, hata/ret metni |
| `ScreenPointer` | tel→PC | mutlak konum x,y ∈ [0,1] + monitör (tık/kaydırma mevcut `Pc` komutlarıyla) |

Video, oturum başına **tek** tek yönlü QUIC akışıdır: 8 bayt `VIDEO_MAGIC` (dosya akışlarının 8 baytlık `id` başlığından ayrışır), sonra
`pts_us:u64 | flags:u8 (bit0=IDR) | len:u32 | Annex-B`. Datagram kullanılmaz (büyük IDR'ler parçalanırdı).

### Geri basınç (`screen::Gate`)
Kuyrukta bekleyen karelerin pts yayılımı 150 ms'yi aşarsa **anahtar olmayan** kareler atılır; atılınca sonraki IDR'ye kadar tüm P kareleri de atılır
(çözücü bozulmasın) ve kodlayıcıdan IDR istenir (`want_key`). IDR kareler her zaman kabul edilir. `adapt_bitrate`: kuyruk >100 ms → bit hızı ×0,75
(taban hedefin 1/4'ü), <20 ms ve hedefin altındaysa ×1,1 ile geri çıkar; ≥%5 değişimde `CODECAPI_AVEncCommonMeanBitRate` canlı güncellenir.
Telefon tarafında çözücü girişi doluysa kare atılır ve IDR istenir (300 ms'de en çok bir istek).

## Hazır ayarlar
| Ad | Çözünürlük | fps | Bit hızı |
|---|---|---|---|
| Eko | 720p (1280×720) | 15 | 2 Mbps |
| Dengeli | 1080p | 30 | 5 Mbps |
| Akıcı | 1080p | 60 | 8 Mbps |
Hedef boyut kaynak en-boyunu korur, çift sayıya yuvarlanır, büyütme yok (`fit_size`).

## Gecikme bütçesi (tahmini, Dengeli / 30 fps, LAN Wi-Fi)
| Aşama | ms |
|---|---|
| Yakalama (WGC, kare değişince) | 0-33 (kare aralığı) |
| GPU dönüşüm + ölçekleme | 1-2 |
| Donanım H.264 (low-latency, B yok) | 3-8 (yazılım MFT: 10-25) |
| QUIC gönderim (Wi-Fi) | 3-15 |
| Telefon çözme + çizme (KEY_LOW_LATENCY) | 5-15 |
| **Toplam** | **~35-90 ms**; kuyruk 150 ms'yi aşarsa kare atılır, gecikme sınırlı kalır |
Telefondaki "fps · ms" göstergesi, kare varış zamanı − pts farkının en küçük değere göre göreli değeridir (saat senkronu gerektirmez).

## Güvenlik ve kullanıcı deneyimi
- Ayarlar'da **"Uzaktan ekran görüntüleme"** kapatılırsa özellik tamamen devre dışı (ScreenInfo hata döner, hiçbir şey yakalanmaz).
- Her yeni telefon için ilk seferde diyalog: *"<Telefon adı> ekranı görmek istiyor"* — İzin ver / Reddet / Her zaman izin ver (kimlik `settings.json > screenAllowed`).
- Paylaşım sürerken: tepsi simgesinde altın nokta + üstte ortada sürüklenebilir, her zaman üstte **"Ekran paylaşılıyor · Durdur"** hapı.
- Mutlak imleç (`ScreenPointer`) yalnızca paylaşım aktifken işlenir.
- Telefon bağlantısı kopunca, "Durdur"a basılınca veya telefon uygulaması arka plana gidince yakalama durur (telefon `ScreenStop` yollar, yüzey kopunca çözücü bırakılır).

## Bilinen sınırlar / doğrulanmayanlar
- Donanım kodlayıcı yolu bu geliştirme makinesinde testle doğrulandı (12 kare → 12 çıktı, 1 IDR). WGC/DXGI yakalama, canlı QUIC akışı, telefonda çözme ve hareketler **cihazda denenmedi**.
- DXGI Desktop Duplication yedeğinde imleç videoya çizilmez.
- Yazılım MFT yolunda NV12 staging UV ofseti `RowPitch×yükseklik` varsayılır (bazı sürücüler hizalı yükseklik kullanabilir).
- Ctrl/Alt tek başına tuş olarak gönderilmez (protokolde yok); mevcut kısayol tuşları (Kopyala/Yapıştır/Alt+Tab/Win) kullanılabilir.
- Ctrl+Alt+Del mümkün değildir (omit).
