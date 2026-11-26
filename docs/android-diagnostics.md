# Android yerel tanılama

`AppLog`, uygulamanın özel `files/logs` klasöründe `app.log`, `.1`, `.2` dosyalarını tutar. Dosya başına hedef sınır 256 KiB; tek mesaj en fazla 16.384 karakterdir ve sınır aşımı tek kayıtla sınırlıdır. Disk yazımları 256 işlik sınırlı bir kuyrukla arka planda yapılır; dolulukta en eski bekleyen kayıt bırakılır. Kayıtlar diske yazılmadan ve logcat'e gönderilmeden maskelenir. Hata raporuna en fazla 200 yerel satır eklenir; sistem logcat dökümü okunmaz.

Maskeleme en iyi çabadır. Rapor gönderilmeden önce kullanıcıya gösterilir; gönderim otomatik değildir. İsteğe bağlı çökme kaydı da yazılmadan önce maskelenir. Uygulama verilerini temizlemek günlükleri siler. Günlükler ve çökme raporları cihazlar arasında kendiliğinden gönderilmez.

`gradlew testDebugUnitTest`: URL kimlik bilgileri/sorgu/parça, e-posta, kullanıcı yolu, IPv4/IPv6, bearer/anahtar/parola, isteğe bağlı GUID ve uzun belirteç testleri. Sayısal IPv6 kontrolü DNS sorgusu yapmaz.
