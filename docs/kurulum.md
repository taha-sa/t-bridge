# T-Bridge — Kurulum

Telefon ile bilgisayar **aynı Wi‑Fi ağında** olmalı. İnternet bağlantısı gerekmez; veriler ağın dışına çıkmaz.

## 1. Bilgisayar (Windows 10/11)

1. `T-Bridge_0.1.0_x64-setup.exe` dosyasını çalıştır. Windows'un yönetici onayına **Evet** de. Kurulum sırasında güvenlik duvarına
   UDP 47800 için gelen bağlantı kuralı eklenir.
2. T-Bridge açılır ve sistem tepsisine (saatin yanına) yerleşir. Pencereyi kapatınca uygulama tepside çalışmaya devam eder.
   Tamamen kapatmak için tepsi simgesine sağ tıklayıp **Çıkış**'ı seç.
3. Uygulama Windows açılışında gizli başlar. İstersen Ayarlar'dan kapatabilirsin.
4. Telefon PC'yi bulamazsa ağ profilini kontrol et: Ayarlar → Ağ ve İnternet → Wi‑Fi → bağlı ağ → **Özel ağ** seçili olmalı.

## 2. Telefon (Android 10+, Xiaomi / MIUI / HyperOS)

1. `app-release.apk` dosyasını yükle. "Bilinmeyen kaynak" uyarısında izin ver.
2. T-Bridge'i aç. **Bağlantı** sekmesindeki izin listesini sırayla tamamla:
   - **Bildirimler**: kalıcı durum bildirimi ve "Panoyu PC'ye gönder" düğmesi için.
   - **Bildirim erişimi**: bildirimleri PC'de görmek, PC'den kapatmak ve medya kontrolü için.
     MIUI "kısıtlanmış ayar" derse: Ayarlar → Uygulamalar → T-Bridge → sağ üstteki ⋮ → **Kısıtlanmış ayarlara izin ver**.
   - **Pil kısıtlaması yok**: Ayarlar → Uygulamalar → T-Bridge → Pil tasarrufu → **Kısıtlama yok**.
   - **Otomatik başlatma** (Xiaomi'ye özel): Ayarlar → Uygulamalar → T-Bridge → **Otomatik başlatma** açık.
   - İsteğe bağlı: Son uygulamalar ekranında T-Bridge kartını aşağı çekip **kilitle**. MIUI bu durumda uygulamayı temizlemez.
3. **Eşleştirme**: PC'de T-Bridge'in ana sayfasında bir QR kod görünür. Telefonda iki yol var:
   - **QR ile eşleştir**'e dokunup kodu okut, ya da
   - "ağda bulunanlardan seç" listesinden PC'yi seçip ekrandaki **6 haneli kodu** gir.

   Kod 5 dakika geçerlidir. Eşleşme bir kez yapılır; sonra iki taraf birbirini kendiliğinden bulur ve bağlanır.

## 3. Kullanım

| İş | Nasıl |
|---|---|
| PC → telefon pano | Otomatik. PC'de Ctrl+C yaptığında metin telefon panosuna geçer. |
| Telefon → PC pano | Android arka planda panonun okunmasına izin vermediği için tek dokunuş gerekir: kalıcı bildirimdeki **Panoyu PC'ye gönder** düğmesi, **hızlı ayarlar kutucuğu** (bildirim panelini düzenle → "Panoyu PC'ye"), ya da **ana ekran widget'ı**. T-Bridge uygulaması açıkken otomatik gönderilir. |
| Metin paylaşımı | Herhangi bir uygulamada Paylaş → T-Bridge. Metin PC panosuna gider. |
| PC → telefon dosya | Dosyaları PC penceresine sürükle-bırak, ya da tepsi menüsünden **Dosya gönder…**. Gelen dosyalar telefonda `İndirilenler/T-Bridge` klasörüne kaydedilir. |
| Telefon → PC dosya | Paylaş → T-Bridge, ya da uygulamada Dosyalar → **Bilgisayara dosya gönder**. Gelen dosyalar PC'de `İndirilenler\T-Bridge` klasörüne kaydedilir (Ayarlar'dan değiştirilebilir). |
| Kesilen aktarım | Bağlantı koparsa aktarım kaldığı yerden devam eder. Karşı taraf kapalıysa dosya sıraya alınır ve bağlanınca gider. |
| Bildirimler | PC'de **Bildirimler** sekmesinde görünür. Kapatınca telefondan da kalkar. Uygulama bazında sessize alınabilir. |
| Pil | PC'de tepsi ipucunda ve ana sayfada görünür. %15'in altına inince PC uyarı verir. |
| Telefon → PC kumanda | **Kumanda** sekmesinde dokunmatik yüzey, klavye, medya, ses, sunum okları, kilit/uyku/kapatma. |
| PC → telefon kumanda | PC'de **Kumanda** sekmesinde telefonu çaldırma (sessizdeyken de çalar), medya ve ses kontrolü. |

## 4. Sorun giderme

- **Bağlanmıyor**:
  - İki cihazın aynı Wi‑Fi ağında olduğunu kontrol et. Misafir ağları ve "AP izolasyonu" cihazların birbirini görmesini engeller.
  - PC'de ağ profili **Özel** olmalı.
  - Telefonda T-Bridge bildirimi "Bilgisayar bekleniyor" diyorsa uygulamayı bir kez aç. Açılış yeniden bağlanmayı tetikler.
- **Bir süre sonra kopuyor (telefon)**: pil kısıtlaması, otomatik başlatma ve kart kilidi adımlarını tekrar kontrol et.
  MIUI bu ayarlar olmadan arka plandaki uygulamaları kapatır.
- **Paylaş menüsünden kuyruğa alınan dosya gitmedi**: paylaşan uygulamanın verdiği okuma izni T-Bridge servisi çalıştığı sürece geçerlidir.
  Telefon yeniden başlatılırsa bu izin düşer ve dosyanın tekrar paylaşılması gerekir.
  Uygulama içindeki **Bilgisayara dosya gönder** ile seçilen dosyalarda bu sorun yoktur.
- **Eşleşmeyi kaldırmak**: PC'de ana sayfadaki eşleşmiş cihazlar listesinden, telefonda ise Bağlantı sekmesinden kaldırılabilir.
