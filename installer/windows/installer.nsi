!include "MUI2.nsh"
!include "FileFunc.nsh"

; General
Name "Vizier"
OutFile "vizier-installer.exe"
InstallDir "$PROGRAMFILES\Vizier"
InstallDirRegKey HKLM "Software\Vizier" "InstallDir"
RequestExecutionLevel admin

; Interface Settings
!define MUI_ABORTWARNING

; Pages
!insertmacro MUI_PAGE_LICENSE "LICENSE*"
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
    SetOutPath "$INSTDIR"
    
    File "..\..\target\x86_64-pc-windows-msvc\release\vizier.exe"
    
    ; Write install directory to registry
    WriteRegStr HKLM "Software\Vizier" "InstallDir" "$INSTDIR"
    
    ; Add to PATH
    ReadEnvStr $0 "PATH"
    StrCpy $1 $0 1
    StrCmp $1 '"' 0 +2
        ; PATH already contains quotes, don't add more
        StrCpy $2 ""
    Goto +2
        ; Need quotes only if path contains spaces
        StrCpy $2 ""
    
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0;$INSTDIR"
    
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
    ; Remove binary
    Delete "$INSTDIR\vizier.exe"
    Delete "$INSTDIR\Uninstall.exe"
    
    ; Remove from PATH
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
    StrReplace "$0" ";$INSTDIR" "" $0
    StrReplace "$0" "$INSTDIR" "" $0
    WriteRegStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$0"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Vizier"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Vizier"
    
    ; Remove install directory
    RMDir "$INSTDIR"
    
    ; Refresh environment
    ExecWait ' rundll32 sysdm.cpl,EditEnvironmentVariables '
SectionEnd
