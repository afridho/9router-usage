# Privacy Policy for 9Router Usage

**Effective date:** September 29, 2026

9Router Usage is a desktop utility that displays provider quotas and credit balances from a 9Router server selected by the user. This policy explains how the application handles data.

## Data collection

9Router Usage does not collect, sell, rent, share, or transmit personal data, analytics, telemetry, advertising identifiers, or usage statistics to the developer or to advertising services.

## Data sent to your 9Router server

To provide its functionality, the application communicates with the local or remote 9Router server URL configured by the user. The following information may be sent to that server:

- The dashboard password entered by the user, for authentication.
- Normal HTTP request metadata required to access the configured 9Router API.

The application receives authentication status, provider connection details, quota information, credit balances, and usage data from that server. This communication occurs directly between the user's device and the server selected by the user. The privacy and security of a remote 9Router instance are also governed by its operator.

## Local storage

The application stores preferences locally, including the configured server URL, refresh interval, language, notification preferences, provider display preferences, and local credit-budget values. On Windows, these settings are stored under:

```text
%APPDATA%\9RouterUsage\settings.json
```

Dashboard passwords are never written to this settings file. If **Remember password securely** is enabled, the password is stored in Windows Credential Manager and separated by server hostname. If the option is disabled, the application does not retain the password after the session ends. Disconnecting removes the saved credential for the active server.

## Notifications

If enabled, quota and credit alerts are generated locally using the Windows notification system. Notification contents are not sent to the developer.

## Network access

The application only initiates network requests needed to communicate with the user-configured 9Router server. The application does not include advertising, analytics, or tracking SDKs.

## Data retention and deletion

Users can remove stored credentials by disconnecting the active server or by deleting the corresponding entry from Windows Credential Manager. Local preferences can be removed by deleting `%APPDATA%\9RouterUsage` or uninstalling the application and removing its app data through Windows Settings.

## Children's privacy

9Router Usage is a general-purpose utility and is not directed toward children. The application knowingly collects no personal information from children or other users.

## Changes to this policy

This policy may be updated when application behavior changes. Updates will be published in this repository with a revised effective date.

## Contact

For privacy questions or requests, open an issue at:

https://github.com/afridho/9router-usage/issues
