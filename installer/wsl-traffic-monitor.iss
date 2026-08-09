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

#ifndef AppVersion
  #define AppVersion "0.0.0"
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
VersionInfoVersion={#AppVersion}

; Per-user install: no elevation, installs under %LOCALAPPDATA%\Programs.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName}

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
; The tray app has no uninstall hook, so remove the Run entry defensively even
; when the startup task was not selected at install time.
Type: files; Name: "{app}\*.log"

[Code]
{ The application holds an exclusive lock on its own executable while running, and
  redb holds one on the history database. Installing or uninstalling over a live
  instance fails with a file-in-use error, so close it first. The window class name
  is the reliable handle: the process name changes between portable and installed
  builds, and there is no visible main window to find by caption. }

const
  TrayWindowClass = 'WSLTrafficMonitorClass';
  WM_CLOSE = $0010;

function FindWindowByClassName(ClassName: string): HWND;
external 'FindWindowW@user32.dll stdcall delayload';

function PostMessageW(hWnd: HWND; Msg: UINT; wParam: Longint; lParam: Longint): BOOL;
external 'PostMessageW@user32.dll stdcall delayload';

{ Ask a running instance to exit, then wait for the window to disappear.
  Returns True if no instance remains. }
function StopRunningInstance(): Boolean;
var
  Wnd: HWND;
  Waited: Integer;
begin
  Wnd := FindWindowByClassName(TrayWindowClass);
  if Wnd = 0 then
  begin
    Result := True;
    Exit;
  end;

  { WM_CLOSE reaches the utility window's WndProc, which tears down the overlay,
    releases the tray icon and flushes pending history to disk. Killing the process
    would skip the flush and lose up to a minute of recorded usage. }
  PostMessageW(Wnd, WM_CLOSE, 0, 0);

  Waited := 0;
  while (Waited < 10000) and (FindWindowByClassName(TrayWindowClass) <> 0) do
  begin
    Sleep(250);
    Waited := Waited + 250;
  end;

  Result := FindWindowByClassName(TrayWindowClass) = 0;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  if StopRunningInstance() then
    Result := ''
  else
    Result := 'WSL Traffic Monitor is still running and could not be closed automatically.' + #13#10 +
              'Please exit it from the system tray (right-click the icon, then Exit) and run this installer again.';
end;

function InitializeUninstall(): Boolean;
begin
  if not StopRunningInstance() then
  begin
    MsgBox('WSL Traffic Monitor is still running.' + #13#10 +
           'Please exit it from the system tray, then run the uninstaller again.',
           mbError, MB_OK);
    Result := False;
  end
  else
    Result := True;
end;

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
