**Effective Date:** November 10, 2026  
**Developer:** taha (GitHub: [taha-sa](https://github.com/taha-sa))  
**Scope:** T-Suite Application Family (T-Bridge, T-Downloader, T-RamCash, T-DeskTree, T-Desk Viewer)

### 1. Core Principle & Data Sovereignty
T-Suite is an open-source, local-first suite of tools. **No personal data is ever collected, transmitted to, or stored on external servers. There are zero tracking, analytics, telemetry, or advertising libraries.** All operational data remains strictly on your local devices.

### 2. Application Features & Local Processing
* **T-Bridge (Windows & Android):** Communicates strictly over the local Wi-Fi network using QUIC with end-to-end encryption (E2EE). Notification mirroring/replies (Android Notification Listener), file transfers, clipboard synchronization, remote desktop (screen stream transmitted exclusively to the paired phone on the LAN), and Windows 11 webcam emulation operate entirely peer-to-peer within your LAN.
* **T-Downloader:** Windows client + Chrome/Edge extension. Browser cookies are accessed strictly locally for authorized download requests via `yt-dlp`. Cookies are never transmitted off-device or stored externally.
* **T-RamCash & T-DeskTree:** Standalone system maintenance utilities that inspect RAM and local file system structures in memory only. No data is retained or transmitted.
* **T-Desk Viewer:** Offline Android document viewer processing files locally.

### 3. Limited Network Connections
Network connectivity is restricted strictly to:
1. Version checks and release downloads via GitHub (`github.com`).
2. Download requests initiated explicitly by the user.
3. Expo update delivery service for T-Desk Viewer.

### 4. Opt-in Crash Reports
No telemetry or silent crash reports exist. If an error occurs, users may optionally generate an issue report. The report is masked to remove personal identifiers and displayed for user review and approval before being submitted as a public GitHub Issue (subject to GitHub’s Privacy Policy).

### 5. Legal Compliance (GDPR & KVKK)
Under the EU General Data Protection Regulation (GDPR) and the Turkish Data Protection Law (KVKK No. 6698), no personal data is stored, aggregated, or profiled on central infrastructure by the developer.

### 6. Children's Privacy & Contact
T-Suite is not directed at children under the age of 13 (or local legal minimum age). For questions, inquiries, or feedback:  
GitHub Issues & Discussions: [https://github.com/taha-sa](https://github.com/taha-sa)


Android logs are kept as bounded, rotating files in private app storage. Personal-information masking runs before writing; it cannot guarantee removal of every secret. Error reports use at most 200 local application log lines and are sent only after user review. Clearing app data removes the logs.
