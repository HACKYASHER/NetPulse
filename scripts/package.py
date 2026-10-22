"""Create a per-user NSIS installer, portable ZIP, source ZIP and checksums."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, zipfile
ROOT=Path(__file__).resolve().parents[1]
RELEASE=ROOT/'release';PAYLOAD=RELEASE/'NetPulse'

def quote(value):return str(value).replace('$','$$').replace('"','$\\"')

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--archives-only',action='store_true',help='Refresh archives and hashes without recompressing the installer')
    parser.add_argument('--installer-only',action='store_true',help='Build the installer for maintenance tests before final archiving')
    options=parser.parse_args()
    for name in ['README.md','README.ru.md','ARCHITECTURE.md','THIRD-PARTY.md','VERIFICATION.md','VERIFICATION.json']:
        if (ROOT/name).is_file():shutil.copy2(ROOT/name,PAYLOAD/name)
    if (ROOT/'VERIFICATION.json').is_file():shutil.copy2(ROOT/'VERIFICATION.json',RELEASE/'VERIFICATION.json')
    shutil.copytree(ROOT/'licenses',PAYLOAD/'licenses',dirs_exist_ok=True)
    shutil.copytree(ROOT/'docs',PAYLOAD/'docs',dirs_exist_ok=True)
    files=sorted(p for p in PAYLOAD.rglob('*') if p.is_file())
    directories=sorted((p for p in PAYLOAD.rglob('*') if p.is_dir()),key=lambda p:len(p.parts),reverse=True)
    script=ROOT/'.build/installer.nsi'
    deletions='\n'.join('Delete "$INSTDIR\\'+quote(p.relative_to(PAYLOAD)).replace('/','\\')+'"' for p in files)
    deletions+='\n'+'\n'.join('RMDir "$INSTDIR\\'+quote(p.relative_to(PAYLOAD)).replace('/','\\')+'"' for p in directories)
    template=r'''
Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"
Var ServiceResult
!include "nsDialogs.nsh"
!include "LogicLib.nsh"
Var MaintenanceDialog
Var UpdateChoice
Var RepairChoice
Var RemoveChoice
Var ExistingDirectory
Var ExistingVersion
Name "NetPulse"
Caption "NetPulse"
VIProductVersion "1.0.0.0"
VIAddVersionKey "ProductName" "NetPulse"
VIAddVersionKey "FileDescription" "NetPulse VPN client installer"
VIAddVersionKey "ProductVersion" "1.0.0 beta"
VIAddVersionKey "FileVersion" "1.0.0 beta"
VIAddVersionKey "LegalCopyright" "NetPulse contributors, GPL-3.0-or-later"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\NetPulse"
InstallDirRegKey HKCU "Software\NetPulseQt" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\NetPulse.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_WELCOME
Page custom MaintenanceCreate MaintenanceLeave
!insertmacro MUI_PAGE_LICENSE "@LICENSE@"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Russian"
!insertmacro MUI_LANGUAGE "English"
LangString MaintenanceTitle ${LANG_RUSSIAN} "Управление NetPulse"
LangString MaintenanceTitle ${LANG_ENGLISH} "Manage NetPulse"
LangString MaintenanceHint ${LANG_RUSSIAN} "Выберите действие. Подписки, серверы и ваши настройки сохраняются."
LangString MaintenanceHint ${LANG_ENGLISH} "Choose an action. Subscriptions, servers and your settings are preserved."
LangString UpdateLabel ${LANG_RUSSIAN} "Обновить NetPulse из этого установщика"
LangString UpdateLabel ${LANG_ENGLISH} "Update NetPulse using this installer"
LangString RepairLabel ${LANG_RUSSIAN} "Восстановить файлы приложения и службу TUN"
LangString RepairLabel ${LANG_ENGLISH} "Repair application files and the TUN service"
LangString RemoveLabel ${LANG_RUSSIAN} "Удалить NetPulse с компьютера"
LangString RemoveLabel ${LANG_ENGLISH} "Uninstall NetPulse from this computer"

Function MaintenanceCreate
  ReadRegStr $ExistingDirectory HKCU "Software\NetPulseQt" "InstallDir"
  IfFileExists "$ExistingDirectory\NetPulse.exe" 0 maintenance_skip
  ReadRegStr $ExistingVersion HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayVersion"
  !insertmacro MUI_HEADER_TEXT "$(MaintenanceTitle)" "NetPulse $ExistingVersion"
  nsDialogs::Create 1018
  Pop $MaintenanceDialog
  ${NSD_CreateLabel} 0 0 100% 35u "$(MaintenanceHint)"
  Pop $0
  ${NSD_CreateRadioButton} 0 45u 100% 20u "$(UpdateLabel)"
  Pop $UpdateChoice
  ${NSD_CreateRadioButton} 0 77u 100% 20u "$(RepairLabel)"
  Pop $RepairChoice
  ${NSD_CreateRadioButton} 0 109u 100% 20u "$(RemoveLabel)"
  Pop $RemoveChoice
  ${NSD_Check} $UpdateChoice
  nsDialogs::Show
  Return
  maintenance_skip:
  Abort
FunctionEnd
Function MaintenanceLeave
  ${NSD_GetState} $RemoveChoice $0
  ${If} $0 == ${BST_CHECKED}
    ExecWait '"$ExistingDirectory\Uninstall.exe"' $1
    SetErrorLevel $1
    Quit
  ${EndIf}
  StrCpy $INSTDIR $ExistingDirectory
FunctionEnd


Function .onInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/WAITFORCLIENT" $R1
  IfErrors regular_start
  StrCpy $R2 0
  wait_client:
  ReadRegStr $0 HKCU "Software\NetPulseQt" "InstallDir"
  IfFileExists "$0\NetPulse.exe" 0 regular_start
  ExecWait '"$0\NetPulse.exe" --installer-check' $1
  StrCmp $1 0 regular_start
  Sleep 500
  IntOp $R2 $R2 + 1
  IntCmp $R2 60 regular_start wait_client regular_start
  regular_start:
  ReadRegStr $0 HKCU "Software\NetPulseQt" "InstallDir"
  StrCmp $0 "" done
  IfFileExists "$0\NetPulse.exe" 0 done
  ExecWait '"$0\NetPulse.exe" --installer-check' $1
  StrCmp $1 0 done
  IfSilent abort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Завершите NetPulse через меню значка в трее и повторите установку."
  abort:
  SetErrorLevel 10
  Abort
  done:
FunctionEnd

Section "NetPulse"
  SetOutPath "$INSTDIR"
  File /r "@PAYLOAD@\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\NetPulseQt" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayName" "NetPulse"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayVersion" "1.0.0-beta"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "Publisher" "NetPulse"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayIcon" "$INSTDIR\NetPulse.exe"
  StrCmp $EXEPATH "$INSTDIR\NetPulse-Maintenance.exe" maintenance_copied
  CopyFiles /SILENT "$EXEPATH" "$INSTDIR\NetPulse-Maintenance.exe"
  maintenance_copied:
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "ModifyPath" '"$INSTDIR\NetPulse-Maintenance.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoModify" 0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoRepair" 1
  Delete "$DESKTOP\NetPulse Qt.lnk"
  Delete "$SMPROGRAMS\NetPulse Qt\NetPulse Qt.lnk"
  RMDir "$SMPROGRAMS\NetPulse Qt"
  CreateDirectory "$SMPROGRAMS\NetPulse"
  CreateShortcut "$SMPROGRAMS\NetPulse\NetPulse.lnk" "$INSTDIR\NetPulse.exe"
  CreateShortcut "$DESKTOP\NetPulse.lnk" "$INSTDIR\NetPulse.exe"
  ; Default install includes the protected TUN service; elevation stays with
  ; the verified service helper rather than the per-user installer.
  ClearErrors
  ${GetParameters} $R0
  ${GetOptions} $R0 "/NOSERVICE" $R1
  IfErrors install_service service_done
  install_service:
  DetailPrint "Installing NetPulse TUN service (Windows confirmation required)..."
  nsExec::ExecToStack '"$INSTDIR\netpulsed.exe" --installer-service-install'
  Pop $ServiceResult
  Pop $1
  StrCmp $ServiceResult 0 service_success
  DetailPrint "TUN service was not installed. System Proxy remains available."
  IfSilent service_failed
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "TUN service installation did not finish. Allow the Windows prompt to enable TUN. Retry, or Cancel to finish with System Proxy only." IDRETRY install_service
  service_failed:
  WriteRegStr HKCU "Software\NetPulseQt" "TunService" "not-installed"
  Goto service_done
  service_success:
  WriteRegStr HKCU "Software\NetPulseQt" "TunService" "installed"
  DetailPrint "TUN service installed and verified."
  service_done:
SectionEnd

Section "Uninstall"
  ExecWait '"$INSTDIR\NetPulse.exe" --installer-check' $0
  StrCmp $0 0 closed
  IfSilent unabort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Завершите NetPulse через меню значка в трее."
  unabort:
  SetErrorLevel 10
  Abort
  closed:
  nsExec::ExecToStack '"$INSTDIR\netpulsed.exe" --installer-service-remove'
  Pop $0
  Pop $1
  StrCmp $0 0 service_removed
  IfSilent service_abort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Служба NetPulse не удалена. Повторите удаление и подтвердите окно Windows."
  service_abort:
  SetErrorLevel 11
  Abort
  service_removed:
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "NetPulseQt"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt"
  DeleteRegKey HKCU "Software\NetPulseQt"
  Delete "$DESKTOP\NetPulse.lnk"
  Delete "$SMPROGRAMS\NetPulse\NetPulse.lnk"
  RMDir "$SMPROGRAMS\NetPulse"
  Delete "$DESKTOP\NetPulse Qt.lnk"
  Delete "$SMPROGRAMS\NetPulse Qt\NetPulse Qt.lnk"
  RMDir "$SMPROGRAMS\NetPulse Qt"
  Delete "$INSTDIR\NetPulse-Maintenance.exe"
@DELETIONS@
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
'''
    for key,value in {'OUTPUT':RELEASE/'NetPulse-Setup.exe','LICENSE':ROOT/'LICENSE.txt','PAYLOAD':PAYLOAD}.items():template=template.replace('@'+key+'@',quote(value))
    template=template.replace('@DELETIONS@',deletions)
    script.write_text(template,encoding='utf-8-sig')
    nsis=os.environ.get('NSIS_EXE') or shutil.which('makensis')
    if not nsis:
        fallback=ROOT.parent/'ONE/.local/tools/nsis-3.13/makensis.exe'
        if fallback.is_file():nsis=str(fallback)
    if not nsis:raise RuntimeError('Set NSIS_EXE to NSIS 3 makensis.exe')
    if not options.archives_only:subprocess.run([nsis,'/V2',str(script)],check=True)
    if options.installer_only:return
    with zipfile.ZipFile(RELEASE/'NetPulse-Portable.zip','w',zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as archive:
        for file in files:archive.write(file,'NetPulse/'+file.relative_to(PAYLOAD).as_posix())
    with zipfile.ZipFile(RELEASE/'NetPulse-Source.zip','w',zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as archive:
        excluded={'.git','.build','.tools','release','target','__pycache__'}
        for directory,names,filenames in os.walk(ROOT):
            names[:]=sorted(name for name in names if name not in excluded)
            for name in sorted(filenames):
                file=Path(directory)/name;relative=file.relative_to(ROOT)
                if file.suffix.lower() in {'.exe','.dll','.pdb','.obj','.pyc'}:continue
                archive.write(file,relative.as_posix())
    source_archive=ROOT.parent/'ONE/release-v2/NetPulse-Engine-Sources.zip'
    if source_archive.is_file():shutil.copy2(source_archive,RELEASE/source_archive.name)
    marker={'schema':1,'application':'NetPulse','runtime':'qt','platform':'windows-x64','version':'1.0.0-beta','installer':'NetPulse-Setup.exe','sha256':hashlib.sha256((RELEASE/'NetPulse-Setup.exe').read_bytes()).hexdigest()}
    (RELEASE/'netpulse-qt-update-v1.json').write_text(json.dumps(marker,indent=2)+'\n',encoding='utf-8')
    outputs=[RELEASE/'netpulse-qt-update-v1.json',RELEASE/'NetPulse-Setup.exe',RELEASE/'NetPulse-Portable.zip',RELEASE/'NetPulse-Source.zip',PAYLOAD/'NetPulse.exe',PAYLOAD/'netpulsed.exe',PAYLOAD/'NetPulseService.exe']
    outputs += [p for p in [RELEASE/'NetPulse-Engine-Sources.zip',RELEASE/'VERIFICATION.json'] if p.is_file()]
    (RELEASE/'checksums.sha256').write_text('\n'.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.relative_to(RELEASE).as_posix() for p in outputs)+'\n',encoding='ascii')
    print(json.dumps({p.name:p.stat().st_size for p in outputs},indent=2))

if __name__=='__main__':main()
