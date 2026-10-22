"""Create a per-user NSIS installer, portable ZIP, source ZIP and checksums."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, zipfile
ROOT=Path(__file__).resolve().parents[1]
RELEASE=ROOT/'release';PAYLOAD=RELEASE/'NetPulse'

def quote(value):return str(value).replace('$','$$').replace('"','$\\"')

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--archives-only',action='store_true',help='Refresh archives and hashes without recompressing the installer')
    options=parser.parse_args()
    for name in ['README.md','README.ru.md','ARCHITECTURE.md','THIRD-PARTY.md','VERIFICATION.md']:
        if (ROOT/name).is_file():shutil.copy2(ROOT/name,PAYLOAD/name)
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
Name "NetPulse 1.0.0 beta"
OutFile "@OUTPUT@"
InstallDir "$LOCALAPPDATA\Programs\NetPulse Qt"
InstallDirRegKey HKCU "Software\NetPulseQt" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\NetPulse.exe"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "@LICENSE@"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Russian"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ReadRegStr $0 HKCU "Software\NetPulseQt" "InstallDir"
  StrCmp $0 "" done
  IfFileExists "$0\NetPulse.exe" 0 done
  ExecWait '"$0\NetPulse.exe" --installer-check' $1
  StrCmp $1 0 done
  IfSilent abort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Завершите NetPulse Qt через меню значка в трее и повторите установку."
  abort:
  SetErrorLevel 10
  Abort
  done:
FunctionEnd

Section "NetPulse Qt"
  SetOutPath "$INSTDIR"
  File /r "@PAYLOAD@\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\NetPulseQt" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayName" "NetPulse Qt"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayVersion" "1.0.0-beta"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "Publisher" "NetPulse"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "DisplayIcon" "$INSTDIR\NetPulse.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt" "NoRepair" 1
  CreateDirectory "$SMPROGRAMS\NetPulse Qt"
  CreateShortcut "$SMPROGRAMS\NetPulse Qt\NetPulse Qt.lnk" "$INSTDIR\NetPulse.exe"
  CreateShortcut "$DESKTOP\NetPulse Qt.lnk" "$INSTDIR\NetPulse.exe"
SectionEnd

Section "Uninstall"
  ExecWait '"$INSTDIR\NetPulse.exe" --installer-check' $0
  StrCmp $0 0 closed
  IfSilent unabort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Завершите NetPulse Qt через меню значка в трее."
  unabort:
  SetErrorLevel 10
  Abort
  closed:
  nsExec::ExecToStack '"$INSTDIR\netpulsed.exe" --installer-service-remove'
  Pop $0
  Pop $1
  StrCmp $0 0 service_removed
  IfSilent service_abort
  MessageBox MB_OK|MB_ICONEXCLAMATION "Служба NetPulse Qt не удалена. Повторите удаление и подтвердите окно Windows."
  service_abort:
  SetErrorLevel 11
  Abort
  service_removed:
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "NetPulseQt"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt"
  DeleteRegKey HKCU "Software\NetPulseQt"
  Delete "$DESKTOP\NetPulse Qt.lnk"
  Delete "$SMPROGRAMS\NetPulse Qt\NetPulse Qt.lnk"
  RMDir "$SMPROGRAMS\NetPulse Qt"
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
    with zipfile.ZipFile(RELEASE/'NetPulse-Portable.zip','w',zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as archive:
        for file in files:archive.write(file,'NetPulse/'+file.relative_to(PAYLOAD).as_posix())
    with zipfile.ZipFile(RELEASE/'NetPulse-Source.zip','w',zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as archive:
        for file in ROOT.rglob('*'):
            if not file.is_file():continue
            relative=file.relative_to(ROOT)
            if any(p in {'.git','.build','.tools','release','target','__pycache__'} for p in relative.parts):continue
            archive.write(file,relative.as_posix())
    source_archive=ROOT.parent/'ONE/release-v2/NetPulse-Engine-Sources.zip'
    if source_archive.is_file():shutil.copy2(source_archive,RELEASE/source_archive.name)
    outputs=[RELEASE/'NetPulse-Setup.exe',RELEASE/'NetPulse-Portable.zip',RELEASE/'NetPulse-Source.zip',PAYLOAD/'NetPulse.exe',PAYLOAD/'netpulsed.exe',PAYLOAD/'NetPulseService.exe']
    (RELEASE/'checksums.sha256').write_text('\n'.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.relative_to(RELEASE).as_posix() for p in outputs)+'\n',encoding='ascii')
    print(json.dumps({p.name:p.stat().st_size for p in outputs},indent=2))

if __name__=='__main__':main()
