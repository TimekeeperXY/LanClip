!macro NSIS_HOOK_PREINSTALL
  ; Release DLL locks held by the previous LanClip/scrcpy session before
  ; NSIS replaces bundled adb and scrcpy files during an upgrade.
  nsExec::ExecToLog 'taskkill /IM LanClip.exe /T /F'
  nsExec::ExecToLog 'taskkill /IM scrcpy.exe /T /F'
  nsExec::ExecToLog 'taskkill /IM adb.exe /T /F'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="LanClip Clipboard Sync"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="LanClip Clipboard Sync" dir=in action=allow program="$INSTDIR\LanClip.exe" protocol=TCP localport=44778 profile=any'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="LanClip Clipboard Sync" dir=in action=allow program="$INSTDIR\LanClip.exe" protocol=UDP localport=44777 profile=any'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop running binaries before the uninstaller removes their DLLs.
  nsExec::ExecToLog 'taskkill /IM LanClip.exe /T /F'
  nsExec::ExecToLog 'taskkill /IM scrcpy.exe /T /F'
  nsExec::ExecToLog 'taskkill /IM adb.exe /T /F'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="LanClip Clipboard Sync"'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "LanClip"
!macroend
