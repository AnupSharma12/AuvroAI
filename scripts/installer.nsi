!include "MUI2.nsh"
Name "AuvroAI"
OutFile "dist\AuvroAI-Setup.exe"
InstallDir "$PROGRAMFILES64\AuvroAI"
RequestExecutionLevel admin

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File "dist\windows\AuvroAI.exe"
  File "assets\icon.ico"
  CreateShortcut "$DESKTOP\AuvroAI.lnk" "$INSTDIR\AuvroAI.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "$SMPROGRAMS\AuvroAI.lnk" "$INSTDIR\AuvroAI.exe" "" "$INSTDIR\icon.ico"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AuvroAI" \
    "DisplayName" "AuvroAI"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AuvroAI" \
    "UninstallString" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\AuvroAI.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$DESKTOP\AuvroAI.lnk"
  Delete "$SMPROGRAMS\AuvroAI.lnk"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AuvroAI"
SectionEnd
