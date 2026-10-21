; Current helper is staged before touching installed files, including old uninstallers.
Var NetPulseServicePrepared
Function NetPulsePrepareService
  StrCmp $NetPulseServicePrepared "1" prepared
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=NetPulseMaintenance.exe "${__FILEDIR__}\..\..\resources\NetPulseService.exe"
  ClearErrors
  ExecWait '"$PLUGINSDIR\NetPulseMaintenance.exe" --installer-stop' $0
  ${If} ${Errors}
  ${OrIf} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "Could not stop the NetPulse service or UAC was cancelled. Installed files were preserved."
    Abort
  ${EndIf}
  StrCpy $NetPulseServicePrepared "1"
  prepared:
  SetOutPath "$INSTDIR"
FunctionEnd
!macro NSIS_HOOK_PREINSTALL
  Call NetPulsePrepareService
!macroend

; Run in the original user context. The helper captures that user's SID before
; requesting UAC, including when elevation uses a different administrator account.
!macro NSIS_HOOK_POSTINSTALL
  ; Keep the legacy uninstall key for in-place upgrades; show the current name.
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "NetPulse ${VERSION}"
  !insertmacro IsShortcutTarget "$DESKTOP\NetPulse 2.0 Alpha.lnk" "$INSTDIR\NetPulse.exe"
  Pop $0
  ${If} $0 = 1
    Delete "$DESKTOP\NetPulse 2.0 Alpha.lnk"
  ${EndIf}
  !insertmacro IsShortcutTarget "$SMPROGRAMS\NetPulse 2.0 Alpha.lnk" "$INSTDIR\NetPulse.exe"
  Pop $0
  ${If} $0 = 1
    Delete "$SMPROGRAMS\NetPulse 2.0 Alpha.lnk"
  ${EndIf}
  DetailPrint "Installing or upgrading the NetPulse TUN service..."
  ExecWait '"$INSTDIR\NetPulse.exe" --installer-service' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "NetPulse TUN service installation failed or was cancelled. Repair it in Network before using TUN."
    SetErrorLevel 1
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\NetPulse.exe" --installer-service-remove' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "Could not remove the NetPulse TUN service. Uninstallation has been stopped."
    Abort
  ${EndIf}
!macroend
