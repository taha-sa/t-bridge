; T-Bridge kurulum kancaları (kullanıcı başına kurulum: yönetici izni gerekmez).
; Güvenlik duvarı kuralını uygulama ilk açılışta bir kez, onay alarak ekler.
!macro NSIS_HOOK_POSTINSTALL
  ; v0.1 (Köprü) otomatik başlatma kaydı
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kopru"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Telefon kamerası kayıtlıysa (HKLM COM kaydı) kaldır: yalnızca bu adım için UAC istenir; kurulum yine kullanıcı başınadır.
  ReadRegStr $0 HKLM "SOFTWARE\Classes\CLSID\{7B3F1C52-4D2E-4C71-9A0B-5E2D8F6A1C34}\InprocServer32" ""
  StrCmp $0 "" +2 0
  ExecShellWait "runas" "$INSTDIR\${MAINBINARYNAME}.exe" "--camera-unregister" SW_HIDE
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "T-Bridge"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kopru"
!macroend
