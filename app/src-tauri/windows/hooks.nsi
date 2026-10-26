!include WinVer.nsh

!macro NSIS_HOOK_PREINSTALL
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "Shotsmith needs Windows 10 or newer.$\r$\n$\r$\nThis version of Windows cannot install Microsoft Edge WebView2, which the app uses to draw its windows. The app cannot open here."
    Quit
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\shotsmith" "" "URL:Shotsmith"
  WriteRegStr HKCU "Software\Classes\shotsmith" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\shotsmith\shell\open\command" "" '"$INSTDIR\shotsmith.exe" "%1"'
!macroend
