# Uzaktan ekran görüntüleme (PC → telefon)

Telefon, PC ekranını canlı izler; dokunma hareketleri PC'de fare/klavye girdisine çevrilir. Yetenek adı: `screen-share`
(Hello/Welcome `caps`). İki taraf da duyurmadıkça hiçbir `Screen*` mesajı veya video akışı kullanılmaz (eski eşler etkilenmez).
Ölçüm / ayarlama / teşhis için ikinci yetenek: **`screen-tune`** (aşağıda). Yeni telefon + eski PC (ya da tersi) çalışır; `screen-tune`
yoksa eski davranışa (kuyruk tabanlı geri basınç, sabit bit hızı, güvenilir işaretçi mesajı) düşülür.

## Mimari

```
PC (desktop/src-tauri)                                        Telefon (android)
┌───────────────────────────────────────────────┐             ┌──────────────────────────────┐
│ Yakalama  WGC (imleç dahil, çerçevesiz Win11)   │             │ Screen.kt                    │
│           └ yedek: DXGI Desktop Duplication     │             │  onFrame → kuyruk → besleyici │
│ Dönüştür  D3D11 Video İşlemcisi: BGRA→NV12 +    │  QUIC       │  MediaCodec H.264 → SurfaceView│
│           ölçekleme (GPU, en-boy korunur)        │ uni akış    │  LOW_LATENCY, PRIORITY=0,     │
│ Kodla     Media Foundation H.264 MFT (High)     │ ─────────▶  │  OPERATING_RATE, render=true  │
│           donanım→sıfır kopya, yoksa yazılım      │ video       │ ScreenView.kt (hareketler,    │
│ Üretim kapısı: alıcı onayı geride → kare ÜRETME  │ ◀───────── │  görüntü alanı, teşhis katmanı)│
│ Uyarlama: fps → bit hızı → çözünürlük (500 ms)   │ kontrol     └──────────────────────────────┘
└───────────────────────────────────────────────┘ ScreenStart/Stop/Keyframe/Config/Info/Pointer
                                                  + (screen-tune) Ack/Stats/Ping/Pong, Datagram::Abs
```

### Protokol (core/src/proto.rs, screen.rs)
| Mesaj | Yön | İçerik |
|---|---|---|
| `ScreenStart` | tel→PC | monitor, **max_w/max_h = telefonun görüntü alanı (piksel)**, fps, bitrate_kbps (**0 = otomatik**) |
| `ScreenStop` / `ScreenKeyframe` | tel→PC | – |
| `ScreenConfig` | tel→PC | canlı kalite/görüntü alanı değişimi (döndürme, yakınlaştırma, ön ayar). Yakalama oturumu AYNI kalır; yalnız dönüştürücü+kodlayıcı yeniden kurulur, ilk kare IDR |
| `ScreenInfo` | PC→tel | monitör listesi, seçili, aktif mi, hata/ret metni |
| `ScreenNotice` | PC→tel | `secure`: güvenli masaüstü (UAC / Ctrl+Alt+Del / kilit) açık, yakalama geçici durdu; `false` = devam |
| `ScreenPointer` | tel→PC | mutlak konum x,y ∈ [0,1] + monitör (tık/kaydırma mevcut `Pc` komutlarıyla) |
| `ScreenAck` (`screen-tune`) | tel→PC | `pts_us` dahil öncesindeki kareleri aldım — **her kare** (gerçek ağ gecikmesini ölçmenin tek yolu) |
| `ScreenStats` (`screen-tune`) | tel→PC | saniyelik alıcı özeti: alınan/çizilen fps, çözme ms, alındı→çizildi ms, uçtan uca ms, geç atılan, giriş beklemesi, kuyruk, RTT |
| `ScreenPing/Pong` (`screen-tune`) | iki yön | saat senkronu (NTP benzeri; telefon ~1 sn'de bir, başta 150 ms'de bir) |
| `Datagram::Abs` (`screen-tune`) | tel→PC | mutlak işaretçi, QUIC datagramı (sürükleme hareketleri; kayıp olabilir, son konum güvenilir gider) |

Video, oturum başına **tek** tek yönlü QUIC akışıdır: 8 bayt `VIDEO_MAGIC`, sonra
`pts_us:u64 | flags:u8 (bit0=IDR) | len:u32 | Annex-B`. `pts_us` = karenin **yakalandığı** an, `kopru_core::screen::now_us()` saatinde
(PC'de süreç ömrü boyunca monoton). Datagram video için kullanılmaz (büyük IDR'ler parçalanırdı).

## Görüntü alanı → kodlama boyutu (bulanıklığın düzeltmesi)
Telefon görüntüleyicinin **gerçek piksel boyutunu** (kutu; tam ekranda pencere, gömülüyken kart) `ScreenStart/Config.max_w/max_h`
olarak bildirir (yakınlaştırma 1 / 1,5 / 2 ile çarpılır, ön ayar üst sınırına kırpılır). PC `choose_size` ile kaynak en-boyunu
koruyarak bu alana **sığan** boyutu seçer (çift sayı, asla büyütme yok, 3840×2160 sınırı) → telefonda yeniden örnekleme yok, SurfaceView
1:1 gösterir. Dikey telefonda yatay PC ekranı genişliğe sığar (letterbox), örn. 1080×2400 alan + 1920×1080 kaynak → 1080×606.
Döndürme ya da yakınlaştırma sonrası alan >%6 değişirse (veya yön değişirse) 250-450 ms gecikmeli `ScreenConfig` gider.

Ön ayar üst sınırları (eksen başına): Eko 1280×720 · Dengeli/Akıcı 1920×1080 (dikeyde ters çevrilmiş); yakınlaştırma sınırı büyütür.

## Hazır ayarlar ve bit hızı tablosu
| Ad | fps | Üst sınır | Bit hızı (otomatik) |
|---|---|---|---|
| Eko | 15 | 720p | 1080p'ye göre piksel/fps ölçekli: 720p15 ≈ 2,2 Mbps |
| **Dengeli (varsayılan)** | 30 | 1080p | 1080p30 ≈ 8,1 Mbps |
| Akıcı | 60 | 1080p | 1080p60 ≈ 13,1 Mbps |

`auto_bitrate_kbps = piksel × 0,13 bit × 30·(fps/30)^0,7 / 1000`, 400..20000 kbps. Telefon boyutu (1080×606@30 ≈ 2,6 Mbps) buna göre
küçülür. Eski sabit 5 Mbps/1080p30 = 0,08 bit/piksel idi: metin ve ince kenarlar için aç. Açık bit hızı verilirse (eski PC/telefon)
o kullanılır.

## Geri basınç ve üretim kapısı (`screen::Gate`)
Eski tasarım: kodlanan kareler gönderim kuyruğuna girer, yazıcı `write_all` ile akışa yazınca kuyruktan çıkar; kuyruk 150 ms'den
uzunsa P kareleri atılırdı. **Sorun:** QUIC akış/bağlantı pencereleri 8/16 MB olduğundan `write_all` ağ yavaşken bile hemen döner;
kuyruk hep ~0 görünür, hiçbir önlem devreye girmez, görüntü gecikmesi sınırsız büyür.

Yeni (`screen-tune` varsa): her yazılan kare **alıcı onayı (`ScreenAck`) gelene dek "uçuşta"** sayılır. En eski onaysız karenin yaşı
(`lag_ms`) **140 ms'yi aşarsa kodlayıcıya yeni kare verilmez** (kare *üretilmez*: referans zinciri bozulmaz, IDR gerekmez); ağ
toparlanınca en yeni ekran görüntüsü kodlanır ("en yeni kareyi gönder, bayatı kuyruklama"). Ağ tamamen tıkandığında bile en az 2 kare/sn
akar. Yazılmamış kuyruk 450 ms'yi aşarsa (son çare) eski davranış: P'ler atılır, IDR istenir.
`screen-tune` yoksa eski kuyruk tabanlı kural (150 ms) sürer.

## Uyarlanabilir kalite (`screen::Adaptor`, 500 ms'de bir)
Kademe merdiveni: **fps** (60→45→30→20→15; hedef zaten düşükse atlanır) → **bit hızı** (%75, %55) → **çözünürlük** (%75, %50).
Karar (`verdict`): üretilmeyen/atılan kare, telefonun çözücüde attığı kare ya da en büyük teslim gecikmesi >140 ms → *bozuk*;
<80 ms ve sorunsuz → *sağlıklı*. Bozulmada tik başına en çok bir kademe ve kademeler arası ≥1 sn; toparlanmada **8 sağlıklı tik (4 sn)
başına bir kademe**; yukarı çıkıp hemen yeniden bozulursa (çırpınma) toparlanma bekleme süresi ikiye katlanır (en çok 24 sn). fps ya da
boyut değişince zincir yeniden kurulur (≈100 ms, bir IDR); yalnız bit hızı canlı güncellenir.

## Kodlayıcı ayarları (`win/screen/encode.rs`)
Donanım MFT öncelikli: sıfır kopya > CPU kopyalı donanım > yazılım (hibrit dizüstüde MFT başka GPU'daysa sıfır kopya veren aranır).
CBR, **B karesi yok**, `AVLowLatencyMode` + `AVEncCommonLowLatency` + `AVEncCommonRealTime`, **High profil** (reddedilirse Main),
VBV ≈ 300 ms bit hızı, **GOP 10 sn** (periyodik IDR yok; güvenilir QUIC'te kurtarma IDR'si istek üzerine: `ScreenKeyframe`, çözücü hatası,
geri basınç, güvenli masaüstü dönüşü). Yakalama/kodlama iş parçacığı: `timeBeginPeriod(1)` + Win11 zamanlayıcı kısıtı kapalı + MMCSS
"Playback" + yüksek öncelik.

## Telefon çözücüsü (`Screen.kt`)
`onFrame` yalnız kuyruğa koyar (Rust QUIC okumasını bloke etmez); ayrı **besleyici** iş parçacığı çözücüye girer, **çıkış** iş
parçacığı `releaseOutputBuffer(render=true)` ile hemen çizer. `KEY_LOW_LATENCY` (API 30+), `KEY_PRIORITY=0`, `KEY_OPERATING_RATE=240`
(reddedilirse yalın biçime düşer). Kuyruk 6 kareyi aşarsa bayat kareler atılır, kuyrukta anahtar kare varsa ondan devam edilir; giriş
tamponu 12 ms içinde gelmezse kare atılır ve IDR istenir (300 ms'de en çok bir istek). Çözücü istisnası → sıfırla + IDR.
Yön kilidi yok; `configChanges` ile döndürmede Activity yeniden yaratılmaz (oturum kesilmez).

## Teşhis (PC günlüğü + telefon gizli katmanı)
**Telefon:** üstteki "NN fps · NN ms" yazısına **uzun bas** → sol altta **TEŞHİS** katmanı (dokununca kapanır):
- `uçtan uca N ms (en çok M)`: PC yakalama anından telefonda çizime (saat senkronuyla; senkron yoksa "göreli" yazar). Hedef Dengeli'de <90 ms.
- `alınan / çizilen`: ağdan gelen ve ekrana basılan kare/sn. Çizilen < alınan → çözücü yetişemiyor.
- `çözme ms`: çözücü giriş→çıkış. `alındı→çizildi ms`: ağdan alınıştan çizime (kuyruk + çözme).
- `geç atılan / giriş beklemesi / kuyruk`: çözücü doluluğu; sıfırdan farklıysa telefon geride.
- `RTT`: saat senkronu gidiş-dönüş. Yüksekse (≥30 ms) Wi-Fi sorunlu.
- son satır: video boyutu, ön ayar, **istek (kutu)** boyutu: video boyutu ≈ istek boyutu olmalı.
Telefon her saniye `adb logcat -s kopru-screen` ile `[ekran-tel]` satırını da yazar.

**PC** (`log` dosyası, saniyede bir; yalnız paylaşım sürerken):
```
[ekran-pc] yakala 29.8/s | kodla 29.8/s anahtar 0 | yakala→kodlandı 4.2/11.0 ms (çağrı 3.1/9.0) | 8.04 Mbps (hedef 8.10) | teslim gecikmesi 31/62 ms kuyruk 1 uçuşta 38 kB | atlanan 0 atılan 0 | 1920x1080@30 L0
[ekran-tel] alınan 29.8/s çizilen 29.5/s | çözme 6.1 ms alındı→çizildi 9.0 ms | uçtan uca 58/120 ms | geç atılan 0 giriş beklemesi 0 kuyruk 1 | RTT 4.2 ms
```
Yakala/kodla fps: PC üretimi (WGC yalnız ekran değişince kare verir; sabit ekranda düşük olması normaldir). `yakala→kodlandı`:
yakalama anından kodlanmış çıktıya (ortalama/en büyük); donanımda <10 ms, yazılımda 15-40 ms. `teslim gecikmesi`: en eski onaysız kare yaşı
(ortalama/en büyük): >140 ms → `atlanan` artar, `L` kademesi düşer. `L0` = tam kalite, `L>0` = uyarlama devrede.
Sorun avı: önce `uçtan uca`ya bak; `teslim gecikmesi` yüksekse ağ, `yakala→kodlandı` yüksekse PC kodlayıcı (yazılım MFT?), `çözme`/`geç atılan`
yüksekse telefon.

## Bulunan başlıca gecikme / bulanıklık nedenleri (koddan doğrulandı)
1. **Kör geri basınç:** QUIC pencereleri 8/16 MB → `write_all` hemen döner → eski `Gate` kuyruğu hep boş → adaptasyon ve kare atma hiç
   çalışmadı; Wi-Fi dalgalanmasında tampon saniyelerce dolup gecikme birikti (asıl "kasma" nedeni). → alıcı onayı + üretim kapısı.
2. **Windows zamanlayıcı çözünürlüğü:** varsayılan ~15,6 ms; yakalama döngüsü ve kodlayıcı pompası `sleep(1ms)` ile bekliyordu → her
   aşama kare süresi kadar sapıyor, 30 fps tutmuyor, 60 fps imkânsızdı. → `timeBeginPeriod(1)` + MMCSS + öncelik.
3. **Sabit 1080p kodlama + telefonda bilinear ölçekleme:** gömülü görüntüleyici ≈970 px genişlik; 1920 px'lik video SurfaceView'da
   ~2× küçültülüp bulanıyor/aliasing yapıyordu; ayrıca gereksiz bant/kodlama yükü. → kodlama boyutu görüntü alanından (1:1).
4. **Bit hızı aç:** 1080p30'da 5 Mbps (0,08 bit/piksel), Main profil. → tablo (≈8 Mbps), High profil.
5. **Periyodik IDR (GOP = 2 sn):** IDR ≈ 5-10× P boyutunda; her 2 sn'de gecikme/bit hızı sıçraması. → GOP 10 sn + isteğe bağlı IDR.
6. **Telefonda bloke eden besleme:** Rust olay iş parçacığı çözücü girişini 10 ms beklerken QUIC okumasını durduruyordu (alıcı tampon büyür).
   → kuyruk + besleyici iş parçacığı, bayat kare atma.
7. **Dikey/yön:** tam ekran `SENSOR_LANDSCAPE`'e kilitliydi; döndürmede Activity yeniden yaratılınca `ScreenLifecycle.onDispose`
   paylaşımı kapatıyordu. → kilit kaldırıldı, `configChanges`, görüntü alanı yeniden müzakeresi.
8. **Yalnız bit hızını** (kör kuyruğa göre) düşüren uyarlama; fps/çözünürlük hiç değişmiyordu. → merdiven.
9. Ölçüm eksikti (pts kodlama anından alınıyordu, uçtan uca yoktu). → pts yakalama anı + saat senkronu + teşhis.

## Gecikme bütçesi (hedef, Dengeli / 30 fps, LAN Wi-Fi) — cihazda henüz ölçülmedi
| Aşama | ms |
|---|---|
| Yakalama (WGC, kare değişince) | 0-8 (döngü beklemesi) |
| GPU dönüşüm + ölçekleme | 1-2 |
| Donanım H.264 (low-latency, B yok) | 3-8 (yazılım MFT: 10-25) |
| QUIC gönderim (Wi-Fi) | 3-15 |
| Telefon çözme + çizme | 5-15 |
| **Toplam** | **~35-70 ms**; alıcı onayı 140 ms geride kalırsa kare üretimi durur, gecikme sınırlı kalır |

## Güvenlik ve kullanıcı deneyimi
- Ayarlar'da **"Uzaktan ekran görüntüleme"** kapatılırsa özellik tamamen devre dışı (ScreenInfo hata döner, hiçbir şey yakalanmaz).
- Her yeni telefon için ilk seferde diyalog: *"<Telefon adı> ekranı görmek istiyor"* — İzin ver / Reddet / Her zaman izin ver (kimlik `settings.json > screenAllowed`).
- Paylaşım sürerken: tepsi simgesinde altın nokta + üstte ortada sürüklenebilir, her zaman üstte **"Ekran paylaşılıyor · Durdur"** hapı.
- Mutlak imleç (`ScreenPointer` / `Datagram::Abs`) yalnızca paylaşım aktifken işlenir.
- Telefon bağlantısı kopunca, "Durdur"a basılınca veya telefon uygulaması arka plana gidince yakalama durur.

## Bilinen sınırlar / doğrulanmayanlar
- Donanım kodlayıcı yolu bu geliştirme makinesinde testle doğrulandı (12 kare → 12 çıktı, 1 IDR; High profil kabul edildi ya da Main'e düştü).
  WGC/DXGI yakalama, canlı QUIC akışı, telefonda çözme, döndürme ve yeni uyarlama **cihazda denenmedi**; sayılar yukarıdaki teşhisle toplanmalı.
- Tek güvenilir QUIC akışı: paket kaybında kafa-kafaya bekleme (HOL) sürer; üretim kapısı bunu hafifletir ama kaldırmaz.
  Sıradaki adım adayları: kare başına akış + bayat akışı sıfırlama, BBR denetleyici, `send_window`'u videoya göre daraltma.
- Yazılım MFT yolunda NV12 hazırlama dokusundan CPU okuma var (hot path'te kopya); yalnız donanım MFT yoksa kullanılır.
- DXGI Desktop Duplication yedeğinde imleç videoya çizilmez.
- Yazılım MFT yolunda NV12 staging UV ofseti `RowPitch×yükseklik` varsayılır (bazı sürücüler hizalı yükseklik kullanabilir).
- Telefon çözücüsü uyarlanabilir oynatmayı 3840×2160'a kadar kurar; PC bu sınırı aşmaz (dikeyde yükseklik ≤2160).
- Çizim zamanı `releaseOutputBuffer` çağrısıdır (gerçek v-sync ≤1 kare sonra); uçtan uca değer buna göre ~8-16 ms iyimser olabilir.
- Ctrl/Alt tek başına tuş olarak gönderilmez (protokolde yok); mevcut kısayol tuşları kullanılabilir. Ctrl+Alt+Del mümkün değildir.

## Ekran sekmesi ve tus tutma (key-hold)

- Telefon arayuzu: Ekran ve Kamera ayri hedefler; alt cubukta 4 birincil sekme + "Daha fazla" sayfasi. Ekran sekmesinde kalici arac cubugu (mod, klavye, tuslar, kalite, tam ekran). PC'ye bildirilen goruntu alani klavyesiz tam boydur; klavye/tus satiri acilinca goruntu yalnizca kucuk gosterilir (yeniden kodlama yok).
- `key-hold` yetenegi: `KeyDown { key }` / `KeyUp { key }` (KeyCode: Ctrl/Alt/Shift/Win, Esc, Tab, oklar, F1-F12, birkac harf). PC SendInput ile sanal tus + tarama kodu + genisletilmis bayrak gonderir; baglanti kopunca ve telefon ekrandan cikinca (ScreenStop) basili tuslari birakir. Yetenek yoksa telefon eski Key/kisayol mesajlarina doner; Ctrl/Alt/Shift/Win tutma, F tuslari, Geri al ve Win+D pasif kalir.
