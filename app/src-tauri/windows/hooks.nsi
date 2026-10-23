!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\shotsmith" "" "URL:Shotsmith"
  WriteRegStr HKCU "Software\Classes\shotsmith" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\shotsmith\shell\open\command" "" '"$INSTDIR\shotsmith.exe" "%1"'
!macroend
