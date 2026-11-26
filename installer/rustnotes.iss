; Inno Setup Script for RustNotes (Windows x86_64)
; Compile with: ISCC.exe /DMyAppVersion=0.1.0 installer\rustnotes.iss

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#ifndef BuildDir
  #define BuildDir "..\target\release"
#endif

; Derive numeric-only version for Windows PE VersionInfoVersion (strip +build and -prerelease)
#define MyAppVersionNumeric MyAppVersion
#if Pos("+", MyAppVersionNumeric) > 0
  #define MyAppVersionNumeric Copy(MyAppVersionNumeric, 1, Pos("+", MyAppVersionNumeric) - 1)
#endif
#if Pos("-", MyAppVersionNumeric) > 0
  #define MyAppVersionNumeric Copy(MyAppVersionNumeric, 1, Pos("-", MyAppVersionNumeric) - 1)
#endif

; Guarantee pdfium.dll is included at compile time (never build an installer without PDF support)
#if FileExists(AddBackslash(BuildDir) + "pdfium.dll")
  #define PdfiumSource AddBackslash(BuildDir) + "pdfium.dll"
#elif FileExists(AddBackslash(SourcePath) + AddBackslash(BuildDir) + "pdfium.dll")
  #define PdfiumSource AddBackslash(SourcePath) + AddBackslash(BuildDir) + "pdfium.dll"
#elif FileExists(AddBackslash(SourcePath) + "..\pdfium.dll")
  #define PdfiumSource AddBackslash(SourcePath) + "..\pdfium.dll"
#else
  #error "Required pdfium.dll was not found in BuildDir or repository root. Cannot build RustNotes installer without PDF library."
#endif

#define MyAppName "RustNotes"
#define MyAppPublisher "RustNotes Contributors"
#define MyAppURL "https://github.com/rajaopak/RustNotes"
#define MyAppExeName "rustnotes.exe"
#define MyAssocFileExt ".rnote"
#define MyAssocKey StringChange(MyAppName, " ", "") + MyAssocFileExt
#define MyAssocName MyAppName + " Document"

[Setup]
AppId={{D37F8E45-926B-4D3C-85AA-A4F1D7728B48}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} v{#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={code:GetDefaultInstallDir}
UsePreviousAppDir=yes
UsePreviousTasks=yes
DisableDirPage=auto
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
CloseApplications=yes
CloseApplicationsFilter=*.exe,*.dll
RestartApplications=no
OutputDir=..\dist
OutputBaseFilename=RustNotes-Setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\assets\icon.ico
UninstallDisplayName={#MyAppName}
LicenseFile=..\LICENSE
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
VersionInfoVersion={#MyAppVersionNumeric}.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BuildDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion replacesameversion restartreplace
Source: "{#PdfiumSource}"; DestDir: "{app}"; Flags: ignoreversion replacesameversion restartreplace
Source: "..\assets\icon.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\assets\icon.png"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion

[Registry]
; Register .rnote file association cleanly
Root: HKA; Subkey: "Software\Classes\{#MyAssocFileExt}\OpenWithProgids"; ValueType: string; ValueName: "{#MyAssocKey}"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}"; ValueType: string; ValueName: ""; ValueData: "{#MyAssocName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\assets\icon.ico,0"
Root: HKA; Subkey: "Software\Classes\{#MyAssocKey}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: "{#MyAssocFileExt}"; ValueData: ""

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
const
  UninstallRegSubKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{D37F8E45-926B-4D3C-85AA-A4F1D7728B48}_is1';

var
  ExistingFound: Boolean;
  ExistingVersion: String;
  ExistingDir: String;
  ExistingIsMachine: Boolean;
  IsUpgradeMode: Boolean;

function HasCommandLineSwitch(const SwitchName: String): Boolean;
var
  I: Integer;
  Arg: String;
  SlashSw, DashSw: String;
begin
  Result := False;
  SlashSw := '/' + Uppercase(SwitchName);
  DashSw := '-' + Uppercase(SwitchName);
  for I := 1 to ParamCount do
  begin
    Arg := Uppercase(Trim(ParamStr(I)));
    if (Arg = SlashSw) or (Arg = DashSw) or
       (Pos(SlashSw + '=', Arg) = 1) or (Pos(DashSw + '=', Arg) = 1) then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

function IsAllDigits(const S: String): Boolean;
var
  I: Integer;
begin
  if Length(S) = 0 then
  begin
    Result := False;
    Exit;
  end;
  Result := True;
  for I := 1 to Length(S) do
  begin
    if (S[I] < '0') or (S[I] > '9') then
    begin
      Result := False;
      Exit;
    end;
  end;
end;

function NextDotSegment(var S: String): String;
var
  DotPos: Integer;
begin
  DotPos := Pos('.', S);
  if DotPos > 0 then
  begin
    Result := Copy(S, 1, DotPos - 1);
    Delete(S, 1, DotPos);
  end
  else
  begin
    Result := S;
    S := '';
  end;
end;

procedure ParseSemVer(RawVer: String; var Major, Minor, Patch, Build: Integer; var PreRelease: String);
var
  PlusPos, DashPos: Integer;
  CorePart: String;
begin
  RawVer := Trim(RawVer);
  if (Length(RawVer) > 0) and ((RawVer[1] = 'v') or (RawVer[1] = 'V')) then
    Delete(RawVer, 1, 1);

  PlusPos := Pos('+', RawVer);
  if PlusPos > 0 then
    Delete(RawVer, PlusPos, Length(RawVer) - PlusPos + 1);

  DashPos := Pos('-', RawVer);
  if DashPos > 0 then
  begin
    PreRelease := Copy(RawVer, DashPos + 1, Length(RawVer) - DashPos);
    CorePart := Copy(RawVer, 1, DashPos - 1);
  end
  else
  begin
    PreRelease := '';
    CorePart := RawVer;
  end;

  Major := StrToIntDef(NextDotSegment(CorePart), 0);
  Minor := StrToIntDef(NextDotSegment(CorePart), 0);
  Patch := StrToIntDef(NextDotSegment(CorePart), 0);
  Build := StrToIntDef(NextDotSegment(CorePart), 0);
end;

function ComparePreRelease(Pre1, Pre2: String): Integer;
var
  Seg1, Seg2: String;
  Num1, Num2: Integer;
  TextCmp: Integer;
begin
  if (Pre1 = '') and (Pre2 = '') then
  begin
    Result := 0;
    Exit;
  end;
  if Pre1 = '' then
  begin
    Result := 1;
    Exit;
  end;
  if Pre2 = '' then
  begin
    Result := -1;
    Exit;
  end;

  while (Pre1 <> '') or (Pre2 <> '') do
  begin
    if Pre1 = '' then
    begin
      Result := -1;
      Exit;
    end;
    if Pre2 = '' then
    begin
      Result := 1;
      Exit;
    end;

    Seg1 := NextDotSegment(Pre1);
    Seg2 := NextDotSegment(Pre2);

    if IsAllDigits(Seg1) and IsAllDigits(Seg2) then
    begin
      Num1 := StrToIntDef(Seg1, 0);
      Num2 := StrToIntDef(Seg2, 0);
      if Num1 > Num2 then
      begin
        Result := 1;
        Exit;
      end
      else if Num1 < Num2 then
      begin
        Result := -1;
        Exit;
      end;
    end
    else if IsAllDigits(Seg1) then
    begin
      Result := -1;
      Exit;
    end
    else if IsAllDigits(Seg2) then
    begin
      Result := 1;
      Exit;
    end
    else
    begin
      TextCmp := CompareText(Seg1, Seg2);
      if TextCmp > 0 then
      begin
        Result := 1;
        Exit;
      end
      else if TextCmp < 0 then
      begin
        Result := -1;
        Exit;
      end;
    end;
  end;

  Result := 0;
end;

function CompareSemVer(const V1, V2: String): Integer;
var
  Maj1, Min1, Pat1, Bld1: Integer;
  Maj2, Min2, Pat2, Bld2: Integer;
  Pre1, Pre2: String;
begin
  ParseSemVer(V1, Maj1, Min1, Pat1, Bld1, Pre1);
  ParseSemVer(V2, Maj2, Min2, Pat2, Bld2, Pre2);

  if Maj1 <> Maj2 then
  begin
    if Maj1 > Maj2 then Result := 1 else Result := -1;
    Exit;
  end;
  if Min1 <> Min2 then
  begin
    if Min1 > Min2 then Result := 1 else Result := -1;
    Exit;
  end;
  if Pat1 <> Pat2 then
  begin
    if Pat1 > Pat2 then Result := 1 else Result := -1;
    Exit;
  end;
  if Bld1 <> Bld2 then
  begin
    if Bld1 > Bld2 then Result := 1 else Result := -1;
    Exit;
  end;

  Result := ComparePreRelease(Pre1, Pre2);
end;

function QueryInstallFromRoot(const RootKey: Integer; var Ver, InstallDir: String): Boolean;
begin
  Result := False;
  Ver := '';
  InstallDir := '';

  if not RegKeyExists(RootKey, UninstallRegSubKey) then
    Exit;

  if not RegQueryStringValue(RootKey, UninstallRegSubKey, 'DisplayVersion', Ver) then
    Ver := '';
  Ver := Trim(Ver);

  if not RegQueryStringValue(RootKey, UninstallRegSubKey, 'InstallLocation', InstallDir) then
    InstallDir := '';
  if InstallDir = '' then
  begin
    if not RegQueryStringValue(RootKey, UninstallRegSubKey, 'Inno Setup: App Path', InstallDir) then
      InstallDir := '';
  end;
  InstallDir := RemoveBackslashUnlessRoot(Trim(InstallDir));

  // Ignore orphaned uninstall entries whose executable no longer exists on disk
  if (Ver <> '') and ((InstallDir = '') or FileExists(AddBackslash(InstallDir) + '{#MyAppExeName}')) then
    Result := True;
end;

procedure DetectExistingInstallation();
var
  UserFound, MachineFound: Boolean;
  UserVer, UserDir, MachineVer, MachineDir: String;
  UserCmp, MachineCmp: Integer;
begin
  ExistingFound := False;
  ExistingVersion := '';
  ExistingDir := '';
  ExistingIsMachine := False;

  UserFound := QueryInstallFromRoot(HKCU, UserVer, UserDir);
  if not UserFound then
    UserFound := QueryInstallFromRoot(HKCU32, UserVer, UserDir);

  MachineFound := QueryInstallFromRoot(HKLM, MachineVer, MachineDir);
  if not MachineFound then
    MachineFound := QueryInstallFromRoot(HKLM32, MachineVer, MachineDir);

  if UserFound and MachineFound then
  begin
    // Prioritize downgrade protection if either scope already has a newer version installed
    UserCmp := CompareSemVer('{#MyAppVersion}', UserVer);
    MachineCmp := CompareSemVer('{#MyAppVersion}', MachineVer);
    if (MachineCmp < 0) and ((UserCmp >= 0) or (CompareSemVer(MachineVer, UserVer) > 0)) then
    begin
      ExistingFound := True;
      ExistingVersion := MachineVer;
      ExistingDir := MachineDir;
      ExistingIsMachine := True;
    end
    else if UserCmp < 0 then
    begin
      ExistingFound := True;
      ExistingVersion := UserVer;
      ExistingDir := UserDir;
      ExistingIsMachine := False;
    end
    else if IsAdminInstallMode then
    begin
      ExistingFound := True;
      ExistingVersion := MachineVer;
      ExistingDir := MachineDir;
      ExistingIsMachine := True;
    end
    else
    begin
      ExistingFound := True;
      ExistingVersion := UserVer;
      ExistingDir := UserDir;
      ExistingIsMachine := False;
    end;
  end
  else if UserFound then
  begin
    ExistingFound := True;
    ExistingVersion := UserVer;
    ExistingDir := UserDir;
    ExistingIsMachine := False;
  end
  else if MachineFound then
  begin
    ExistingFound := True;
    ExistingVersion := MachineVer;
    ExistingDir := MachineDir;
    ExistingIsMachine := True;
  end;
end;

function GetDefaultInstallDir(Param: String): String;
begin
  if ExistingFound and (ExistingDir <> '') then
    Result := ExistingDir
  else
    Result := ExpandConstant('{autopf}\{#MyAppName}');
end;

function InitializeSetup(): Boolean;
var
  Cmp: Integer;
  LocationText: String;
  UserDataDir: String;
  Msg: String;
begin
  IsUpgradeMode := False;
  DetectExistingInstallation();

  if not ExistingFound then
  begin
    Result := True;
    Exit;
  end;

  if ExistingDir <> '' then
    LocationText := ExistingDir
  else
    LocationText := '(direktori instalasi standar)';

  UserDataDir := ExpandConstant('{localappdata}\RustNotes');
  Cmp := CompareSemVer('{#MyAppVersion}', ExistingVersion);

  // 1. Downgrade blocked: installer is older than currently installed version
  if Cmp < 0 then
  begin
    Log('Downgrade blocked: installed v' + ExistingVersion + ', installer v{#MyAppVersion}.');
    if not WizardSilent then
    begin
      Msg :=
        'PERINGATAN: Versi installer tidak mutakhir (outdated)!' + #13#10#13#10 +
        'RustNotes versi lebih baru (v' + ExistingVersion + ') sudah terpasang di:' + #13#10 +
        '  ' + LocationText + #13#10#13#10 +
        'Installer ini adalah versi lebih lama (v{#MyAppVersion}).' + #13#10 +
        'Demi menghindari bug dan masalah kompatibilitas catatan, JANGAN menjalankan installer versi lama di atas versi yang lebih baru.' + #13#10#13#10 +
        'Instalasi dibatalkan secara otomatis.';
      MsgBox(Msg, mbCriticalError, MB_OK);
    end;
    Result := False;
    Exit;
  end;

  // 2. Same version already installed: notify user and exit cleanly unless /REINSTALL
  if Cmp = 0 then
  begin
    if not HasCommandLineSwitch('REINSTALL') then
    begin
      Log('Same version already installed: v' + ExistingVersion + ' at ' + LocationText + '.');
      if not WizardSilent then
      begin
        Msg :=
          'RustNotes v' + ExistingVersion + ' sudah terpasang di Windows Anda.' + #13#10#13#10 +
          'Lokasi terpasang: ' + LocationText + #13#10 +
          'Data catatan Anda di ' + UserDataDir + ' tetap aman.' + #13#10#13#10 +
          'Tidak diperlukan pembaruan atau instalasi ulang.';
        MsgBox(Msg, mbInformation, MB_OK);
      end;
      Result := False;
      Exit;
    end;

    Log('Reinstall explicitly requested via /REINSTALL for v{#MyAppVersion}.');
    IsUpgradeMode := True;
    Result := True;
    Exit;
  end;

  // 3. Newer version: update existing installation in-place
  if not WizardSilent then
  begin
    if ExistingIsMachine and (not IsAdminInstallMode) then
    begin
      Msg :=
        'RustNotes v' + ExistingVersion + ' saat ini terpasang untuk semua pengguna (Administrator) di:' + #13#10 +
        '  ' + LocationText + #13#10#13#10 +
        'Untuk memperbarui ke v{#MyAppVersion} di lokasi yang sama tanpa menduplikasi instalasi, silakan jalankan ulang installer dan pilih "Install for all users".';
      MsgBox(Msg, mbInformation, MB_OK);
      Result := False;
      Exit;
    end;

    if (not ExistingIsMachine) and IsAdminInstallMode then
    begin
      Msg :=
        'RustNotes v' + ExistingVersion + ' saat ini terpasang khusus untuk pengguna ini (Current User) di:' + #13#10 +
        '  ' + LocationText + #13#10#13#10 +
        'Untuk memperbarui ke v{#MyAppVersion} di lokasi yang sama tanpa menduplikasi instalasi, silakan jalankan ulang installer dan pilih "Install for me only".';
      MsgBox(Msg, mbInformation, MB_OK);
      Result := False;
      Exit;
    end;

    Msg :=
      'Pembaruan RustNotes Terdeteksi' + #13#10#13#10 +
      'Versi terpasang : v' + ExistingVersion + #13#10 +
      'Versi installer : v{#MyAppVersion}' + #13#10 +
      'Lokasi aplikasi : ' + LocationText + #13#10#13#10 +
      'Installer akan memperbarui aplikasi dan pustaka PDF (pdfium.dll) ke versi terbaru.' + #13#10 +
      'Semua catatan (notes), pengaturan, dan template Anda di:' + #13#10 +
      '  ' + UserDataDir + #13#10 +
      'TIDAK akan dihapus atau diubah.' + #13#10#13#10 +
      'Lanjutkan pembaruan sekarang?';
    if MsgBox(Msg, mbConfirmation, MB_YESNO or MB_DEFBUTTON1) <> IDYES then
    begin
      Result := False;
      Exit;
    end;
  end;

  IsUpgradeMode := True;
  Log('Updating RustNotes from v' + ExistingVersion + ' to v{#MyAppVersion} at ' + LocationText + '.');
  Result := True;
end;

procedure InitializeWizard();
begin
  if IsUpgradeMode then
  begin
    WizardForm.WelcomeLabel1.Caption :=
      'Memperbarui RustNotes ke v{#MyAppVersion}';
    WizardForm.WelcomeLabel2.Caption :=
      'Installer akan memperbarui RustNotes dari v' + ExistingVersion + ' ke v{#MyAppVersion} (termasuk pustaka PDFium).' + #13#10#13#10 +
      'Semua catatan, pengaturan, dan template Anda di %LOCALAPPDATA%\RustNotes tetap aman dan tidak tersentuh.' + #13#10#13#10 +
      'Klik Next untuk melanjutkan pembaruan.';
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExePath, PdfDllPath: String;
begin
  if CurStep = ssPostInstall then
  begin
    ExePath := ExpandConstant('{app}\{#MyAppExeName}');
    PdfDllPath := ExpandConstant('{app}\pdfium.dll');
    Log('Post-install check: exe=' + ExePath + ' (exists=' + IntToStr(Ord(FileExists(ExePath))) + '), pdfium=' + PdfDllPath + ' (exists=' + IntToStr(Ord(FileExists(PdfDllPath))) + ').');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  UserDataDir: String;
begin
  // Preserve user notes and settings by default; only offer explicit opt-in deletion
  // during interactive uninstallation.
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent) then
  begin
    UserDataDir := ExpandConstant('{localappdata}\RustNotes');
    if DirExists(UserDataDir) then
    begin
      if MsgBox(
        'Do you also want to permanently delete your personal RustNotes notebooks, settings, and logs in:' + #13#10 +
        UserDataDir + #13#10#13#10 +
        'Select NO to keep your notes for future installations.',
        mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      begin
        DelTree(UserDataDir, True, True, True);
      end;
    end;
  end;
end;
