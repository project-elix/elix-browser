!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\ELIXBrowserHTML" "" "ELIX Browser HTML"
  WriteRegStr HKCU "Software\Classes\ELIXBrowserHTML" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\ELIXBrowserHTML\DefaultIcon" "" '"$INSTDIR\elix-browser.exe",0'
  WriteRegStr HKCU "Software\Classes\ELIXBrowserHTML\shell\open\command" "" '"$INSTDIR\elix-browser.exe" --url "%1"'
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser" "" "ELIX Browser"
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\DefaultIcon" "" '"$INSTDIR\elix-browser.exe",0'
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\shell\open\command" "" '"$INSTDIR\elix-browser.exe"'
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\Capabilities" "ApplicationName" "ELIX Browser"
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\Capabilities" "ApplicationDescription" "ELIX Browser"
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\Capabilities\URLAssociations" "http" "ELIXBrowserHTML"
  WriteRegStr HKCU "Software\Clients\StartMenuInternet\ELIX Browser\Capabilities\URLAssociations" "https" "ELIXBrowserHTML"
  WriteRegStr HKCU "Software\RegisteredApplications" "ELIX Browser" "Software\Clients\StartMenuInternet\ELIX Browser\Capabilities"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\RegisteredApplications" "ELIX Browser"
  DeleteRegKey HKCU "Software\Clients\StartMenuInternet\ELIX Browser"
  DeleteRegKey HKCU "Software\Classes\ELIXBrowserHTML"
!macroend
