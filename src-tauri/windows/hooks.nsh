!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="LanClip Clipboard Sync"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="LanClip Clipboard Sync" dir=in action=allow program="$INSTDIR\LanClip.exe" protocol=TCP localport=44778 profile=any'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="LanClip Clipboard Sync" dir=in action=allow program="$INSTDIR\LanClip.exe" protocol=UDP localport=44777 profile=any'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="LanClip Clipboard Sync"'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "LanClip"
!macroend
