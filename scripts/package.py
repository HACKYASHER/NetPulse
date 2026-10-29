"""Create a protected, single-root NSIS installer, portable ZIP, source ZIP and checksums."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, zipfile
ROOT=Path(__file__).resolve().parents[1]
RELEASE=ROOT/'release';PAYLOAD=RELEASE/'NetPulse'

def quote(value):return str(value).replace('$','$$').replace('"','$\\"')
def validate_payload(directory):
    for path in directory.rglob('*'):
        relative=path.relative_to(directory)
        if relative.parts[0].lower()=='data' or path.name.lower()=='client.lock' or path.name.lower().endswith(('.db','.db-wal','.db-shm')):
            raise RuntimeError('Personal data must never be packaged: '+relative.as_posix())

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--prepare-only',action='store_true',help='Refresh payload metadata and NSIS script without compiling or archiving')
    parser.add_argument('--archives-only',action='store_true',help='Refresh archives and hashes without recompressing the installer')
    parser.add_argument('--installer-only',action='store_true',help='Build the installer for maintenance tests before final archiving')
    options=parser.parse_args()
    validate_payload(PAYLOAD)
    # The Qt build keeps the QML sources embedded in the executable. Refuse a
    # stale payload if an interface file was edited after that build.
    for source in list((ROOT/'qml').glob('*.qml'))+list((ROOT/'qml').glob('*.js')):
        embedded=ROOT/'.build/qt/NetPulse/qml'/source.name
        if not embedded.is_file() or embedded.read_bytes()!=source.read_bytes():
            raise RuntimeError('Rebuild NetPulse before packaging changed QML: '+source.name)
    for name in ['README.md','README.ru.md','ARCHITECTURE.md','THIRD-PARTY.md','VERIFICATION.md','VERIFICATION.json']:
        if (ROOT/name).is_file():shutil.copy2(ROOT/name,PAYLOAD/name)
    if (ROOT/'VERIFICATION.json').is_file():shutil.copy2(ROOT/'VERIFICATION.json',RELEASE/'VERIFICATION.json')
    shutil.copytree(ROOT/'licenses',PAYLOAD/'licenses',dirs_exist_ok=True)
    shutil.copytree(ROOT/'docs',PAYLOAD/'docs',dirs_exist_ok=True)
    # The normal user's first launch uses this bounded list to remove the old
    # per-user program payload, leaving unknown/personal files untouched.
    manifest=sorted(p.relative_to(PAYLOAD).as_posix() for p in PAYLOAD.rglob('*') if p.is_file())
    manifest=sorted(set(manifest+['Uninstall.exe','NetPulse-Maintenance.exe','installation-files.txt']))
    (PAYLOAD/'installation-files.txt').write_text('\n'.join(manifest)+'\n',encoding='utf-8')
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
VIProductVersion "1.0.5.0"
VIAddVersionKey "ProductName" "NetPulse"
VIAddVersionKey "FileDescription" "NetPulse VPN client installer"
VIAddVersionKey "ProductVersion" "1.0.5"
VIAddVersionKey "FileVersion" "1.0.5"
VIAddVersionKey "LegalCopyright" "NetPulse contributors, GPL-3.0-or-later"
OutFile "@OUTPUT@"
InstallDir "$PROGRAMFILES64\NetPulse"
InstallDirRegKey HKLM "Software\NetPulseQt" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
SetCompressorDictSize 32
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\NetPulse.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_WELCOME
Page custom MaintenanceCreate MaintenanceLeave
!insertmacro MUI_PAGE_LICENSE "@LICENSE@"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE InstallDirectoryLeave
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
LangString ServiceInstalling ${LANG_RUSSIAN} "Проверка и установка службы TUN NetPulse..."
LangString ServiceInstalling ${LANG_ENGLISH} "Checking and installing the NetPulse TUN service..."
LangString ServiceInstalled ${LANG_RUSSIAN} "Служба TUN установлена и проверена."
LangString ServiceInstalled ${LANG_ENGLISH} "The TUN service is installed and verified."
LangString ServiceUnavailable ${LANG_RUSSIAN} "Клиент установлен, но TUN недоступен. Код обслуживания: $ServiceResult."
LangString ServiceUnavailable ${LANG_ENGLISH} "The client is installed, but TUN is unavailable. Maintenance code: $ServiceResult."
LangString ServiceFilesBlocked ${LANG_RUSSIAN} "Компоненты службы отсутствуют или заблокированы. Проверьте отчёт антивируса. Повторная установка файлов сейчас отменена; клиент установлен без готовой службы TUN."
LangString ServiceFilesBlocked ${LANG_ENGLISH} "Service components are missing or blocked. Check the antivirus report. File installation has stopped; the client is installed without a ready TUN service."
LangString ServiceDamaged ${LANG_RUSSIAN} "Windows ещё видит старую службу NetPulse, но её регистрация повреждена. Завершите обработку обнаружения антивирусом, закройте клиент и установщик, затем перезапустите Windows. Повторная установка службы сейчас отменена."
LangString ServiceDamaged ${LANG_ENGLISH} "Windows still lists the old NetPulse service, but its registration is damaged. Finish the antivirus remediation, close the client and installer, then restart Windows. Service installation has stopped."
LangString ServiceRestart ${LANG_RUSSIAN} "Windows не подтвердила остановку или удаление службы. Закройте NetPulse и установщик. Если состояние сохраняется, перезапустите Windows; файлы службы сейчас не заменяются."
LangString ServiceRestart ${LANG_ENGLISH} "Windows has not confirmed that the service stopped or was removed. Close NetPulse and the installer. If this persists, restart Windows; service files are not being replaced."
LangString ServiceCancelled ${LANG_RUSSIAN} "Подтверждение Windows отменено. Нажмите «Повтор», чтобы снова запросить установку службы, или «Отмена», чтобы завершить установку клиента без готового TUN."
LangString ServiceCancelled ${LANG_ENGLISH} "Windows confirmation was cancelled. Retry to request service installation again, or Cancel to finish installing the client without a ready TUN service."
LangString ServiceFailed ${LANG_RUSSIAN} "Служба TUN не установлена (код $ServiceResult). Это не обязательно связано с подтверждением Windows. Клиент установлен; причина ошибки сохранена в подробностях установки."
LangString ServiceFailed ${LANG_ENGLISH} "The TUN service was not installed (code $ServiceResult). This does not necessarily mean Windows confirmation was cancelled. The client is installed; the error is recorded in the installation details."

Function MaintenanceCreate
  ReadRegStr $ExistingDirectory HKLM "Software\NetPulseQt" "InstallDir"
  IfFileExists "$ExistingDirectory\NetPulse.exe" 0 maintenance_skip
  ReadRegStr $ExistingVersion HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayVersion"
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


Function InstallDirectoryLeave
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-check-location "$INSTDIR"'
  Pop $0
  Pop $1
  StrCmp $0 0 location_valid
  MessageBox MB_OK|MB_ICONEXCLAMATION "Не удалось использовать выбранную папку / The selected folder cannot be used.$\r$\n$1"
  Abort
  location_valid:
FunctionEnd

Function .onInit
  SetRegView 64
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=netpulsed.exe "@PAYLOAD@\netpulsed.exe"
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/WAITFORCLIENT" $R1
  IfErrors regular_start
  StrCpy $R2 0
  wait_client:
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-check-client'
  Pop $1
  Pop $0
  StrCmp $1 0 regular_start
  Sleep 500
  IntOp $R2 $R2 + 1
  IntCmp $R2 60 regular_start wait_client regular_start
  regular_start:
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-check-client'
  Pop $1
  Pop $0
  StrCmp $1 0 done
  IfSilent abort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Завершите NetPulse через меню значка в трее и повторите установку / Quit NetPulse from the tray before installing."
  abort:
  SetErrorLevel 10
  Abort
  done:
FunctionEnd

Section "NetPulse"
  ; Validate and protect the canonical root before any payload overwrite.
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=netpulsed.exe "@PAYLOAD@\netpulsed.exe"
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-prepare-layout "$INSTDIR"'
  Pop $0
  Pop $1
  StrCmp $0 0 layout_ready
  IfSilent layout_abort
  MessageBox MB_OK|MB_ICONSTOP "Не удалось подготовить общую папку NetPulse / Could not prepare the NetPulse folder.$\r$\n$1"
  layout_abort:
  SetErrorLevel 12
  Abort
  layout_ready:
  SetOutPath "$INSTDIR"
  File /r "@PAYLOAD@\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  StrCmp $EXEPATH "$INSTDIR\NetPulse-Maintenance.exe" maintenance_copied
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-cache "$INSTDIR" "$EXEPATH"'
  Pop $0
  Pop $1
  StrCmp $0 0 maintenance_copied
  IfSilent cache_abort
  MessageBox MB_OK|MB_ICONSTOP "Не удалось сохранить установщик NetPulse / Could not preserve the installer.$\r$\n$1"
  cache_abort:
  SetErrorLevel 12
  Abort
  maintenance_copied:
  nsExec::ExecToStack '"$PLUGINSDIR\netpulsed.exe" --installer-finalize-layout "$INSTDIR"'
  Pop $0
  Pop $1
  StrCmp $0 0 files_protected
  IfSilent security_abort
  MessageBox MB_OK|MB_ICONSTOP "Не удалось защитить файлы NetPulse / Could not protect NetPulse files.$\r$\n$1"
  security_abort:
  SetErrorLevel 12
  Abort
  files_protected:
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayName" "NetPulse"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayVersion" "1.0.5"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "Publisher" "NetPulse"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayIcon" "$INSTDIR\NetPulse.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "ModifyPath" '"$INSTDIR\NetPulse-Maintenance.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoModify" 0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoRepair" 1
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
  DetailPrint "$(ServiceInstalling)"
  nsExec::ExecToStack '"$INSTDIR\netpulsed.exe" --installer-service-install'
  Pop $ServiceResult
  Pop $1
  StrCmp $ServiceResult 0 service_success
  DetailPrint "$(ServiceUnavailable)"
  DetailPrint "$1"
  IfSilent service_failed
  StrCmp $ServiceResult 20 service_files_blocked
  StrCmp $ServiceResult 21 service_restart
  StrCmp $ServiceResult 22 service_damaged
  StrCmp $ServiceResult 23 service_cancelled
  StrCmp $ServiceResult 24 service_restart
  MessageBox MB_OK|MB_ICONEXCLAMATION "$(ServiceFailed)"
  Goto service_failed
  service_files_blocked:
  MessageBox MB_OK|MB_ICONEXCLAMATION "$(ServiceFilesBlocked)"
  Goto service_failed
  service_damaged:
  MessageBox MB_OK|MB_ICONEXCLAMATION "$(ServiceDamaged)"
  Goto service_failed
  service_restart:
  MessageBox MB_OK|MB_ICONEXCLAMATION "$(ServiceRestart)"
  Goto service_failed
  service_cancelled:
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(ServiceCancelled)" IDRETRY install_service
  service_failed:
  WriteRegStr HKLM "Software\NetPulseQt" "TunService" "not-installed"
  WriteRegStr HKLM "Software\NetPulseQt" "TunServiceError" "$ServiceResult"
  StrCmp $ServiceResult "error" service_failure_unknown
  StrCmp $ServiceResult "timeout" service_failure_unknown
  SetErrorLevel $ServiceResult
  Goto service_done
  service_failure_unknown:
  SetErrorLevel 29
  Goto service_done
  service_success:
  SetErrorLevel 0
  WriteRegStr HKLM "Software\NetPulseQt" "TunService" "installed"
  DeleteRegValue HKLM "Software\NetPulseQt" "TunServiceError"
  DetailPrint "$(ServiceInstalled)"
  service_done:
  StrCmp $ServiceResult 0 cleanup_previous cleanup_finished
  cleanup_previous:
  nsExec::ExecToStack '"$INSTDIR\netpulsed.exe" --installer-cleanup-previous'
  Pop $0
  Pop $1
  DetailPrint "$1"
  cleanup_finished:
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt"
  DeleteRegKey HKCU "Software\NetPulseQt"
SectionEnd

Section "Uninstall"
  SetRegView 64
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
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt"
  ; Keep the protected profile-location identity for reinstall/migration.
  ; The Apps & Features entry above is removed normally.
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
    if options.prepare_only:return
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
    source_archive=RELEASE/'NetPulse-Engine-Sources.zip'
    if not source_archive.is_file():raise RuntimeError('Run scripts/fetch-engine-sources.py before packaging')
    with zipfile.ZipFile(source_archive) as source_zip:
        source_manifest=json.loads(source_zip.read('manifest.json'))
        if {(r['name'],r['tag']) for r in source_manifest['roots']}!={('sing-box','v1.14.4-netpulse.1'),('xray','v26.10.21')}:raise RuntimeError('Engine sources do not match the bundled versions')
    marker={'schema':1,'application':'NetPulse','runtime':'qt','platform':'windows-x64','version':'1.0.5','installer':'NetPulse-Setup.exe','sha256':hashlib.sha256((RELEASE/'NetPulse-Setup.exe').read_bytes()).hexdigest()}
    (RELEASE/'netpulse-qt-update-v1.json').write_text(json.dumps(marker,indent=2)+'\n',encoding='utf-8')
    outputs=[RELEASE/'netpulse-qt-update-v1.json',RELEASE/'NetPulse-Setup.exe',RELEASE/'NetPulse-Portable.zip',RELEASE/'NetPulse-Source.zip',PAYLOAD/'NetPulse.exe',PAYLOAD/'netpulsed.exe',PAYLOAD/'NetPulseService.exe']
    outputs += [p for p in [RELEASE/'NetPulse-Engine-Sources.zip',RELEASE/'VERIFICATION.json'] if p.is_file()]
    (RELEASE/'checksums.sha256').write_text('\n'.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.relative_to(RELEASE).as_posix() for p in outputs)+'\n',encoding='ascii')
    print(json.dumps({p.name:p.stat().st_size for p in outputs},indent=2))

if __name__=='__main__':main()
