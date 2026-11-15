; nsExec interprets child UTF-8 through the ANSI code page. Do not interpolate
; its captured output into dialogs; the helper writes UTF-8 details separately.
!macro FlyxServiceError
  ${If} $LANGUAGE == 1049
    MessageBox MB_ICONSTOP "Не удалось выполнить операцию со службой VPN Flyx. Код: $0.$\r$\n$\r$\nПодробности: $PROGRAMFILES64\FlyxService\install-error.log"
  ${Else}
    MessageBox MB_ICONSTOP "Could not configure the Flyx VPN service. Code: $0.$\r$\n$\r$\nDetails: $PROGRAMFILES64\FlyxService\install-error.log"
  ${EndIf}
  Abort
!macroend

!macro NSIS_HOOK_PREINSTALL
  InitPluginsDir
  File /oname=$PLUGINSDIR\flyx-service-helper.exe "$%FLYX_SERVICE_HELPER%"
  nsExec::ExecToStack '"$PLUGINSDIR\flyx-service-helper.exe" unregister'
  Pop $0
  Pop $1
  ${If} $0 != 0
    !insertmacro FlyxServiceError
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToStack '"$INSTDIR\flexconnect-service.exe" register'
  Pop $0
  Pop $1
  ${If} $0 != 0
    !insertmacro FlyxServiceError
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  InitPluginsDir
  CopyFiles /SILENT "$INSTDIR\flexconnect-service.exe" "$PLUGINSDIR\flyx-service-helper.exe"
  nsExec::ExecToStack '"$PLUGINSDIR\flyx-service-helper.exe" uninstall'
  Pop $0
  Pop $1
  ${If} $0 != 0
    !insertmacro FlyxServiceError
  ${EndIf}
!macroend
