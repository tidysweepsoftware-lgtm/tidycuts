; Inno Setup script for the TidyCuts installer.
; Built by .github/workflows/release.yml after the program and FFmpeg have
; been put together in dist\TidyCuts:
;   iscc /DAppVersion=1.3.1 packaging\windows\tidycuts.iss
; The installer is written to dist\TidyCuts-Setup-<version>.exe.

#ifndef AppVersion
  #define AppVersion "1.3.1"
#endif

[Setup]
AppId={{6F3A6A52-9C1B-4B5E-8F0A-2D9B7C4E1A10}
AppName=TidyCuts
AppVersion={#AppVersion}
AppPublisher=TidySweep Software
DefaultDirName={autopf}\TidyCuts
DefaultGroupName=TidyCuts
DisableProgramGroupPage=yes
OutputDir=..\..\dist
OutputBaseFilename=TidyCuts-Setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
SetupIconFile=..\icons\tidycuts.ico
UninstallDisplayIcon={app}\TidyCuts.ico
ChangesAssociations=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
Name: "projectfiles"; Description: "Open TidyCuts projects (.tcproj) with TidyCuts"

[Files]
; The program, the ffmpeg folder with its licence, and the README.
Source: "..\..\dist\TidyCuts\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion createallsubdirs

[Icons]
Name: "{autoprograms}\TidyCuts"; Filename: "{app}\TidyCuts.exe"; IconFilename: "{app}\TidyCuts.ico"
Name: "{autodesktop}\TidyCuts"; Filename: "{app}\TidyCuts.exe"; IconFilename: "{app}\TidyCuts.ico"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.tcproj"; ValueType: string; ValueData: "TidyCuts.Project"; Flags: uninsdeletevalue; Tasks: projectfiles
Root: HKA; Subkey: "Software\Classes\TidyCuts.Project"; ValueType: string; ValueData: "TidyCuts project"; Flags: uninsdeletekey; Tasks: projectfiles
Root: HKA; Subkey: "Software\Classes\TidyCuts.Project\DefaultIcon"; ValueType: string; ValueData: "{app}\TidyCuts.ico"; Tasks: projectfiles
Root: HKA; Subkey: "Software\Classes\TidyCuts.Project\shell\open\command"; ValueType: string; ValueData: """{app}\TidyCuts.exe"" ""%1"""; Tasks: projectfiles

[Run]
Filename: "{app}\TidyCuts.exe"; Description: "Start TidyCuts"; Flags: nowait postinstall skipifsilent
