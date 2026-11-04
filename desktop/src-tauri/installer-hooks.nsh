; Köprü kurulum kancaları: Windows Güvenlik Duvarı'nda uygulamaya gelen UDP izni (QUIC + mDNS).
!macro NSIS_HOOK_POSTINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="Köprü"'
  nsExec::Exec 'netsh advfirewall firewall add rule name="Köprü" dir=in action=allow protocol=UDP program="$INSTDIR\kopru-desktop.exe" profile=private,public enable=yes'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="Köprü"'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kopru"
!macroend
