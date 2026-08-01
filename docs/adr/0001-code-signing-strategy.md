# ADR 0001: Code Signing Strategy for Windows Release Binaries

- **Status**: Proposed
- **Date**: 2026-08-01
- **Deciders**: Core Architecture Team
- **Technical Story**: Windows Tray Application Trust & SmartScreen Friction

## Context and Problem Statement

`wsl-traffic-monitor` is a native Windows system tray application distributed as a standalone binary (`wsl-traffic-monitor.exe`). Currently, release binaries produced by GitHub Actions CI are unsigned.

When users download and run unsigned executables on Windows 10/11:
1. **Windows Defender SmartScreen** displays a prominent warning banner ("Windows protected your PC – Unknown Publisher").
2. **User Experience & Trust** are severely degraded, as users must click "More info" $\rightarrow$ "Run anyway".
3. **Enterprise / Managed Environments** may block unsigned binaries completely under AppLocker / WDAC policies.

To achieve production v1.0 readiness, release binaries must be digitally signed with a trusted Authenticode certificate.

## Decision Drivers

- **Automated CI/CD Integration**: Signing must execute non-interactively within the `.github/workflows/release.yml` GitHub Actions runner.
- **SmartScreen Reputation**: Certificate must build or establish immediate SmartScreen reputation without requiring physical hardware tokens.
- **Cost Efficiency**: Sustainable for an open-source project.
- **Key Security**: Private signing keys must never be stored directly in source code or unencrypted environment variables.

## Options Evaluated

### Option 1: Azure Trusted Signing (Recommended)

Microsoft's cloud-native code signing service integrated directly into Windows and GitHub Actions.

- **Pros**:
  - Native integration with GitHub Actions via `azure/trusted-signing-action` or `signtool.exe`.
  - Immediate Windows SmartScreen trust as a recognized Microsoft partner service.
  - No physical hardware token required (HSM key protection managed by Azure Key Vault backend).
  - Cost-effective subscription model (~$10/month for open source/indie developers).
- **Cons**:
  - Requires Microsoft Azure active subscription and identity validation.

### Option 2: Traditional EV (Extended Validation) Certificate on Hardware Token

Traditional Code Signing Certificate issued by DigiCert / Sectigo.

- **Pros**:
  - Immediate SmartScreen reputation bypass.
- **Cons**:
  - Requires physical USB hardware token (HSM), making automated headless GitHub Actions CI runner integration complex (requires remote KeyLocker HSM proxy).
  - High annual cost ($300–$600/year).

### Option 3: Self-Signed Authenticode Certificate with User Import Script

Generating a project-specific CA and signing release binaries locally.

- **Pros**:
  - Free and fully automated in local build scripts.
- **Cons**:
  - Does NOT bypass SmartScreen on end-user machines unless users manually import the project Root CA into Windows Trusted Root Store.
  - Unsuitable for public production releases.

## Recommended Decision

Adopt **Option 1: Azure Trusted Signing**.

### Integration Steps for v1.0:

1. Provision an Azure Trusted Signing Account & Identity Validation profile.
2. Store Azure credentials (`AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`) as GitHub Repository Secrets.
3. Update `.github/workflows/release.yml` step before artifact zipping:
   ```yaml
   - name: Sign Executable via Azure Trusted Signing
     uses: azure/trusted-signing-action@v1
     with:
       azure-tenant-id: ${{ secrets.AZURE_TENANT_ID }}
       azure-client-id: ${{ secrets.AZURE_CLIENT_ID }}
       azure-client-secret: ${{ secrets.AZURE_CLIENT_SECRET }}
       endpoint: https://eus.codesigning.azure.net/
       code-signing-account-name: wsl-traffic-monitor-signing
       certificate-profile-name: ReleaseSigningProfile
       files-folder: target/release
       files-folder-filter: wsl-traffic-monitor.exe
   ```
4. Verify digital signature in CI using `signtool verify /pa /v target/release/wsl-traffic-monitor.exe`.
