# Signing the downloads

Windows shows "unknown publisher" for programs that are not signed, and a Mac says an
app "can't be checked for malware" until it is signed and notarized by Apple. Both
need a certificate that costs money and belongs to the publisher, so the build can
sign the files **only once you have added the certificate** as secrets. Until then
the installers are built exactly as before.

## Windows

1. Buy a code-signing certificate (an OV or EV certificate from any certificate
   authority; EV removes the SmartScreen warning at once, OV builds trust over time).
2. Export it as a `.pfx` file with a password.
3. Turn the file into one line of text: `base64 -w0 cert.pfx` (or
   `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))` in PowerShell).
4. In the repository: Settings > Secrets and variables > Actions > New repository
   secret. Add `WINDOWS_CERT_PFX_BASE64` (that text) and `WINDOWS_CERT_PASSWORD`.

The next run of the **Installers** workflow signs `TidyCuts.exe` and the installer.
The Microsoft Store version needs none of this: the Store signs it.

## Mac

1. Join the Apple Developer Program and create a **Developer ID Application**
   certificate. Export it from Keychain Access as a `.p12` file with a password.
2. Make an app-specific password for your Apple ID (appleid.apple.com).
3. Add these secrets: `MACOS_CERT_P12_BASE64` (`base64 -i cert.p12`),
   `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` (the certificate's name, for example
   `Developer ID Application: Your Name (TEAMID)`), `APPLE_ID`, `APPLE_TEAM_ID` and
   `APPLE_APP_PASSWORD`.

The workflow then signs the app, sends it to Apple to be notarized and attaches the
result to the disk image.

None of the signing steps have been run yet, because no certificate has been added.
The first run with a certificate may need a small fix.
