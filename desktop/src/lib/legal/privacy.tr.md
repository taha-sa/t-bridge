**Yürürlük Tarihi:** 10 Kasım 2026  
**Geliştirici:** taha (GitHub: [taha-sa](https://github.com/taha-sa))  
**Kapsam:** T-Suite yazılım ailesi (T-Bridge, T-Downloader, T-RamCash, T-DeskTree, T-Desk Viewer)

### 1. Temel İlke ve Veri Egemenliği
T-Suite açık kaynaklı, şeffaf ve yerel öncelikli (local-first) bir yazılım projesidir. **Hiçbir uzak sunucuya kişisel veri aktarılmaz; telemetri, analitik, reklam veya kullanıcı izleme mekanizmaları kesinlikle yer almaz.** Tüm verileriniz cihazınızda veya yerel ağınızda kalır.

### 2. Uygulama Bazlı İzinler ve Veri İşleme
* **T-Bridge (Windows & Android):** Cihazlar aynı yerel Wi-Fi ağında QUIC protokolü ile uçtan uca şifreli (E2EE) haberleşir. Bildirim yansıtma ve yanıtlama (Android Bildirim Erişimi), yerel dosya aktarımı, pano eşitleme, uzaktan masaüstü (ekran akışı yalnızca yerel ağda eşleşmiş telefona verilir) ve Windows 11 webcam işlevleri yalnızca eşleşmiş cihazlar arasında doğrudan çalışır; üçüncü taraflarla paylaşılmaz.
* **T-Downloader:** Windows istemcisi ve Chrome/Edge tarayıcı eklentisinden oluşur. Kimlik doğrulama gerektiren indirmelerde oturum çerezleri yalnızca yerel indirme isteği sırasında `yt-dlp` motoru tarafından kullanılır. Çerezler dışa aktarılmaz veya saklanmaz.
* **T-RamCash & T-DeskTree:** Tamamen yerel çalışan sistem yardımcı araçlarıdır. RAM ve disk meta verilerini sisteminizde yerel olarak işler; kaydetmez veya iletmez.
* **T-Desk Viewer:** Android üzerindeki belgeleri tamamen çevrimdışı görüntüler.

### 3. Ağ Erişimi Sınırları
Uygulamalar yalnızca aşağıdaki durumlarda ağa erişir:
1. GitHub üzerinden sürüm ve güncelleme kontrolü (`github.com`).
2. Kullanıcının T-Downloader üzerinden bizzat başlattığı indirme işlemleri.
3. T-Desk Viewer güncellemeleri için Expo güncelleme servisi.

### 4. Hata Raporlama (İsteğe Bağlı)
Bir çökme veya hata meydana geldiğinde otomatik rapor gönderilmez. Kullanıcı hata bildirmek isterse, kişisel verilerden arındırılmış ve maskelenmiş rapor metnini önceden inceler. Kullanıcı onay verirse rapor herkese açık GitHub Issue olarak oluşturulur; bu aşamadan itibaren GitHub'ın kendi Gizlilik Politikası geçerlidir.

### 5. Yasal Dayanak (KVKK & GDPR) ve Haklar
6698 sayılı Kişisel Verilerin Korunması Kanunu (KVKK) ve AB Genel Veri Koruma Tüzüğü (GDPR) kapsamında veri sorumlusu sıfatıyla merkezi bir kişisel veri işleme faaliyeti yürütülmemektedir. Veriler kullanıcı kontrolündeki yerel ortamlardadır.

### 6. Çocukların Gizliliği ve İletişim
T-Suite 13 yaşın (veya yargı bölgesine göre ilgili yaş sınırının) altındaki çocuklara yönelik değildir. Soru ve bildirimleriniz için iletişim adresi:  
GitHub Issues / Discussions: [https://github.com/taha-sa](https://github.com/taha-sa)


Android günlükleri uygulamanın özel dosya alanında sınırlı ve dönen dosyalar halinde saklanır. Yazılmadan önce kişisel bilgi maskelemesi uygulanır; bu işlem tüm gizli bilgilerin kaldırılacağını garanti etmez. Hata raporu yalnızca uygulamanın en fazla 200 yerel günlük satırını kullanır ve kullanıcı incelemesinden sonra gönderilir. Uygulama verilerini temizlemek günlükleri kaldırır.
