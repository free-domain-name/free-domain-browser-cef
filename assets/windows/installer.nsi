; NSIS installer for the portable Windows package.
; Invoke from the repository root:
;   makensis /DSTAGE=<abs stage> /DOUTFILE=<abs output.exe> /DVERSION=<version> assets/windows/installer.nsi
; The version comes from Cargo.toml, so the installer never says a version
; of its own.
Unicode true
!include "MUI2.nsh"

!ifndef STAGE
  !define STAGE "target\package\FreeDomainBrowser-CEF-windows-x64"
!endif
!ifndef OUTFILE
  !define OUTFILE "target\FreeDomainBrowser-CEF-windows-x64-setup.exe"
!endif
!ifndef VERSION
  !define VERSION "0.0.0"
!endif

Name "Free Domain Browser"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Free Domain Browser"
InstallDirRegKey HKCU "Software\FreeDomainBrowserCEF" "InstallDir"
RequestExecutionLevel user

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Free Domain Browser" SecMain
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*"
  CreateShortCut "$DESKTOP\Free Domain Browser.lnk" "$INSTDIR\free-domain-browser-cef.exe"
  CreateShortCut "$SMPROGRAMS\Free Domain Browser.lnk" "$INSTDIR\free-domain-browser-cef.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\FreeDomainBrowserCEF" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF" "DisplayName" "Free Domain Browser"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF" "DisplayIcon" "$INSTDIR\free-domain-browser-cef.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF" "Publisher" "Free Domain Browser contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF" "DisplayVersion" "${VERSION}"
SectionEnd

Section "Uninstall"
  RMDir /r "$INSTDIR"
  Delete "$DESKTOP\Free Domain Browser.lnk"
  Delete "$SMPROGRAMS\Free Domain Browser.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeDomainBrowserCEF"
  DeleteRegKey HKCU "Software\FreeDomainBrowserCEF"
SectionEnd
