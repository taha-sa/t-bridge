; T-Bridge kurulum kancaları (kullanıcı başına kurulum: yönetici izni gerekmez).
; Güvenlik duvarı kuralını uygulama ilk açılışta bir kez, onay alarak ekler.
!macro NSIS_HOOK_POSTINSTALL
  ; v0.1 (Köprü) otomatik başlatma kaydı
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kopru"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "T-Bridge"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kopru"
!macroend
