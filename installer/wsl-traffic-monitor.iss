; Inno Setup script for WSL Traffic Monitor.
;
; Per-user install by design: the application writes autostart to HKCU and settings
; to %APPDATA%, so it needs no administrator rights. Installing per-user avoids a UAC
; prompt entirely and keeps uninstall clean.
;
; This also fixes a real defect in the portable distribution: autostart recorded the
; path of wherever the user happened to unzip the executable, so moving the folder
; silently broke startup with no error surfaced. An installed location is stable.
;
; Built in CI by .github/workflows/release.yml. To build locally you need Inno Setup 6:
;   iscc /DAppVersion=0.6.0 /DSourceExe=path\to\wsl-traffic-monitor.exe wsl-traffic-monitor.iss
;
; AppVersion is the display string and may carry a prerelease suffix (0.6.1-rc1).
; NumericVersion feeds the Win32 VERSIONINFO resource, which accepts only dotted
; numbers -- passing a prerelease string there fails compilation outright.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#ifndef NumericVersion
  #define NumericVersion "0.0.0"
#endif

#ifndef SourceExe
  #define SourceExe "..\target\release\wsl-traffic-monitor.exe"
#endif

#define AppName "WSL Traffic Monitor"
#define AppPublisher "WSL Traffic Monitor contributors"
#define AppUrl "https://github.com/mattDev0/wsl-traffic-monitor"
#define AppExeName "wsl-traffic-monitor.exe"

[Setup]
; AppId must never change between versions or upgrades will install alongside
; instead of replacing.
AppId={{8F3A6C21-7D4E-4B92-A5C6-1E9B7D2F4A83}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#NumericVersion}

; Per-user install: no elevation, installs under %LOCALAPPDATA%\Programs.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}

; The application creates this mutex as its single-instance guard. Inno checks for
; it during install and uninstall and asks the user to close the app if present,
; which avoids failing on a locked executable or a redb database still held open.
;
; This replaces a hand-written [Code] routine that called FindWindowW to post
; WM_CLOSE. That routine declared FindWindowW with one parameter when it takes two;
; under stdcall the callee cleans the stack, so setup corrupted its own stack and
; crashed before extracting anything. Inno's own check does the job without any
; hand-declared FFI.
AppMutex=WslTrafficMonitorSingleInstanceMutex

LicenseFile=..\LICENSE
OutputDir=.\dist
OutputBaseFilename=wsl-traffic-monitor-{#AppVersion}-setup
SetupIconFile=..\apps\wsl-traffic-monitor\assets\app.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; WSL2 requires Windows 10 1903 or later; refuse to install anywhere the app
; could not work.
MinVersion=10.0.18362

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "startupicon"; Description: "Start {#AppName} when I sign in"; GroupDescription: "Startup:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
; The application manages this key itself via its "Run at Startup" tray toggle.
; Writing it here honours the installer checkbox; uninstall removes it so we do
; not leave a Run entry pointing at a deleted executable.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "WslTrafficMonitor"; \
    ValueData: """{app}\{#AppExeName}"""; \
    Flags: uninsdeletevalue; Tasks: startupicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName} now"; \
    Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Anything the application writes beside itself at runtime. The Run registry value
; is removed by the uninsdeletevalue flag above, and user data under %APPDATA% is
; handled by CurUninstallStepChanged so it can be kept if the user wants it.
Type: files; Name: "{app}\*.log"

[Code]
{ Only Inno built-ins are used here. Declaring Windows API functions by hand in
  this section is how an earlier revision crashed setup, so the running-instance
  check is handled by the AppMutex directive above instead. }

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: string;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    { Settings and usage history are user data, so removal is opt-in rather than
      automatic. Reinstalling should not silently discard months of history. }
    DataDir := ExpandConstant('{userappdata}\wsl-traffic-monitor');
    if DirExists(DataDir) then
    begin
      if MsgBox('Remove saved settings and usage history?' + #13#10 +
                'Choose No to keep them for a future reinstall.',
                mbConfirmation, MB_YESNO) = IDYES then
        DelTree(DataDir, True, True, True);
    end;
  end;
end;
