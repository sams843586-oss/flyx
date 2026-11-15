!macro NSIS_HOOK_PREINSTALL
  InitPluginsDir
  File /oname=$PLUGINSDIR\flyx-service-helper.exe "$%FLYX_SERVICE_HELPER%"
  nsExec::ExecToStack '"$PLUGINSDIR\flyx-service-helper.exe" unregister'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Could not stop the Flyx VPN service: $1"
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToStack '"$INSTDIR\flexconnect-service.exe" register'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Could not register the Flyx VPN service: $1"
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToStack '"$INSTDIR\flexconnect-service.exe" unregister'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Could not stop the Flyx VPN service: $1"
    Abort
  ${EndIf}
!macroend
