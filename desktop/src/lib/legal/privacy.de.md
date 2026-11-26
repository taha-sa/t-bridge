**Gültig ab:** 10. November 2026  
**Entwickler:** taha (GitHub: [taha-sa](https://github.com/taha-sa))  
**Geltungsbereich:** T-Suite Anwendungsfamilie (T-Bridge, T-Downloader, T-RamCash, T-DeskTree, T-Desk Viewer)

### 1. Grundprinzip und Datensparsamkeit
T-Suite ist eine quelloffene, lokale Software-Suite (Local-First). **Es werden keinerlei personenbezogene Daten an externe Server übermittelt. Es findet kein Tracking, keine Telemetrie und keine Analyse des Nutzerverhaltens statt.** Alle Daten verbleiben auf Ihren lokalen Geräten.

### 2. Funktionsbezogene Datenverarbeitung
* **T-Bridge (Windows & Android):** Verbindung ausschließlich im selben lokalen WLAN mittels Ende-zu-Ende-verschlüsseltem (E2EE) QUIC-Protokoll. Benachrichtigungsspiegelung & Direktantworten (Android-Benachrichtigungszugriff), Dateiübertragung, Zwischenablage-Abgleich, Remote-Desktop (Bildschirmübertragung ausschließlich an das gekoppelte Smartphone im lokalen Netz) sowie die Webcam-Funktion für Windows 11 verbleiben strikt im lokalen Netzwerk.
* **T-Downloader:** Windows-Anwendung + Chrome/Edge-Erweiterung. Browser-Cookies werden ausschließlich lokal für autorisierte Download-Anfragen über `yt-dlp` genutzt und weder weitergeleitet noch extern gespeichert.
* **T-RamCash & T-DeskTree:** Lokale Systemwerkzeuge zur temporären Arbeitsspeicher- und Speicherplatzanalyse. Es erfolgt keine Aufzeichnung oder Übertragung.
* **T-Desk Viewer:** Lokaler Android-Dokumentenbetrachter ohne Cloud-Anbindung.

### 3. Netzwerkzugriffe
Netzwerkverbindungen beschränken sich ausschließlich auf:
1. Versionsprüfung und Downloads von Updates über GitHub (`github.com`).
2. Vom Nutzer explizit gestartete Downloadvorgänge.
3. Den Expo-Aktualisierungsdienst für T-Desk Viewer.

### 4. Fehlerberichte (Opt-in)
Es gibt keine automatisierte Fehlerübermittlung. Sollte ein Fehler auftreten, kann der Nutzer freiwillig einen Fehlerbericht erstellen. Der Bericht wird anonymisiert/maskiert, dem Nutzer zur Prüfung angezeigt und erst nach ausdrücklicher Freigabe als öffentliches GitHub-Issue eingestellt (hierbei gilt die Datenschutzerklärung von GitHub).

### 5. DSGVO- & KVKK-Konformität
Gemäß der EU-Datenschutz-Grundverordnung (DSGVO) und dem türkischen Datenschutzgesetz (KVKK Nr. 6698) werden vom Entwickler keine personenbezogenen Daten serverseitig verarbeitet oder gespeichert.

### 6. Jugendschutz & Kontakt
Die Anwendungen richten sich nicht an Kinder unter 13 bzw. 16 Jahren. Für Fragen und Anliegen:  
GitHub Issues / Discussions: [https://github.com/taha-sa](https://github.com/taha-sa)


Android-Protokolle werden als begrenzte, rotierende Dateien im privaten App-Speicher abgelegt. Personenbezogene Daten werden vor dem Schreiben maskiert. Dies garantiert nicht die Entfernung aller vertraulichen Informationen. Fehlerberichte verwenden höchstens 200 lokale App-Protokollzeilen und werden erst nach Prüfung durch den Benutzer gesendet. Das Löschen der App-Daten entfernt die Protokolle.
