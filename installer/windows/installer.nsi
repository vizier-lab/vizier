!include "MUI2.nsh"
!include "WordFunc.nsh"
!include "WinMessages.nsh"

; General
Name "Vizier"
OutFile "vizier-installer.exe"
InstallDir "$PROGRAMFILES64\Vizier"
InstallDirRegKey HKLM "Software\Vizier" "InstallDir"
RequestExecutionLevel admin

; Interface Settings
!define MUI_ABORTWARNING

; Pages
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "English"

; Version info
VIProductVersion "0.3.0.0"
VIAddVersionKey "ProductName" "Vizier"
VIAddVersionKey "CompanyName" "Vizier"
VIAddVersionKey "FileDescription" "Vizier Installer"
VIAddVersionKey "FileVersion" "0.3.0"
VIAddVersionKey "ProductVersion" "0.3.0"
VIAddVersionKey "LegalCopyright" "Copyright 2024"

; Installation section
Section "Install"
    SetRegView 64
    SetOutPath "$INSTDIR"

    File "..\..\target\x86_64-pc-windows-msvc\release\vizier.exe"

    ; Write install directory to registry
    WriteRegStr HKLM "Software\Vizier" "InstallDir" "$INSTDIR"

    ; Add to PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0;$INSTDIR"

    ; Notify the system of the PATH change
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Write uninstall info to Add/Remove Programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "DisplayName" "Vizier"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "Publisher" "Vizier"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "DisplayVersion" "0.3.0"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier" "NoRepair" 1

SectionEnd

; Uninstallation section
Section "Uninstall"
    SetRegView 64

    ; Remove binary
    Delete "$INSTDIR\vizier.exe"
    Delete "$INSTDIR\Uninstall.exe"

    ; Remove install directory from PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    ${WordReplace} "$0" ";$INSTDIR" "" "+" $0
    ${WordReplace} "$0" "$INSTDIR;" "" "+" $0
    ${WordReplace} "$0" "$INSTDIR" "" "+" $0
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0"

    ; Notify the system of the PATH change
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

    ; Remove registry keys
    DeleteRegKey HKLM "Software\Vizier"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier"

    ; Remove install directory
    RMDir "$INSTDIR"

SectionEnd
