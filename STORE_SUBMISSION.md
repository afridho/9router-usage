# Microsoft Store submission

## Product identity

| Field | Value |
|---|---|
| Package Identity Name | `afridho.9RouterUsage` |
| Publisher | `CN=0D463717-DA8E-4AA8-B8B8-3CFD70863B03` |
| Publisher display name | `DEZ Labs` |
| Store ID | `9NR91D484XF6` |
| Store URL | https://apps.microsoft.com/detail/9NR91D484XF6 |
| Package Family Name | `afridho.9RouterUsage_tk50stqxnjnp0` |

These values are case-sensitive and are already included in `src-tauri/store/AppxManifest.xml.template`.

## Build the Store package

Install the Windows 10 or Windows 11 SDK, then run in Windows PowerShell:

```powershell
npm ci
.\scripts\build-msix.ps1
```

The output is:

```text
dist\9Router-Usage-1.0.1-store-x64.msix
```

The MSIX is intentionally unsigned. Upload it directly to Partner Center; Microsoft signs it after certification. It cannot be sideloaded normally until it is signed.

The fourth version component is always `0`, as required for Store submissions. Increase the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` before publishing an update.

## Partner Center listing

Recommended values:

- Category: **Utilities & tools**
- Privacy policy URL: `https://github.com/afridho/9router-usage/blob/master/PRIVACY.md`
- Support URL: `https://github.com/afridho/9router-usage/issues`
- Website: `https://github.com/afridho/9router-usage`
- Store languages: English and Indonesian

Upload screenshots that accurately represent the current application. Explain that the app requires a separately running local or remote 9Router instance.

## Certification checklist

1. Build from a clean checkout on Windows x64.
2. Confirm that the app opens, displays the tray icon, and can be opened from it.
3. Verify local and remote server login, refresh, logout, notifications, and Credential Manager storage.
4. Verify single-instance behavior.
5. Test installation and launch with an MSIX package signed by a temporary development certificate, if desired.
6. Run the Windows App Certification Kit against the package.
7. Upload the unsigned Store MSIX to the reserved product in Partner Center.
8. Complete age ratings, properties, privacy declaration, pricing, markets, descriptions, icons, and screenshots.
9. Submit for certification.

## Important notes

- The Store package targets Windows Desktop x64, minimum build `10.0.17763.0`.
- The package declares `runFullTrust`, internet client, and private-network access because this is a desktop tray app that connects to user-configured local or remote servers.
- Autostart and Windows notifications should be retested after Store installation because packaged app behavior can differ from an unpackaged installation.
- The GitHub EXE/MSI/portable artifacts remain separate from the Store MSIX distribution.
