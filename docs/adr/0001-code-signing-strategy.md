# ADR 0001: Code Signing Strategy for Windows Release Binaries

- **Status**: Accepted — infrastructure provisioned, blocked on identity validation
- **Date**: 2026-08-01 (provisioned 2026-08-09)
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

## Implementation Status

### Provisioned (2026-08-09)

| Resource | Value |
|---|---|
| Subscription | `820beeb9-30c6-41bf-b6a8-8765f9e35d83` (Azure for Students Starter) |
| Resource group | `rg-wsl-traffic-monitor` (eastus) |
| Signing account | `wsl-traffic-signing`, SKU **Basic** |
| Account endpoint | `https://eus.codesigning.azure.net/` |
| GitHub Actions identity | App registration `wsl-traffic-monitor-github-actions` |
| Role | Artifact Signing Certificate Profile Signer, scoped to the signing account |

The release workflow authenticates by **OIDC federation**, not a client secret. A
federated credential on the app registration trusts the subject
`repo:mattDev0/wsl-traffic-monitor:environment:release`, which is why the release job
declares `environment: release` and `permissions: id-token: write`.

This supersedes step 2 of the original plan. `AZURE_CLIENT_SECRET` is deliberately
absent: with federation there is no long-lived secret to store, leak or rotate. The
three values held as environment secrets (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`,
`AZURE_SUBSCRIPTION_ID`) are identifiers rather than credentials — they grant nothing
without a token issued to that exact repository and environment.

### Blocked: identity validation

No certificate profile exists yet, so **releases are still published unsigned**.
Creating one requires an `identityValidationId`, and identity validation is a manual
Microsoft review that cannot be driven from the CLI or any API:

```
PUT .../certificateProfiles/release
-> ObjectMissingRequiredProperty: identityValidationId
```

To complete it:

1. Azure Portal → the `wsl-traffic-signing` account → **Identity validation** → start a
   request. Individuals verify with government ID; organisations submit legal entity
   documents and need 3+ years of verifiable history.
2. Wait for Microsoft approval (typically 1–7 business days).
3. Create a **public trust** certificate profile named `release` — the name the workflow
   expects via `SIGNING_PROFILE`. If a different name is used, update that variable.
4. Re-run the release workflow. It detects the credentials and signs automatically.

### Behaviour before validation completes

The workflow does not fail without signing. It checks whether `AZURE_CLIENT_ID` is
present and, if not, emits a CI warning, skips the signing and verification steps, and
labels the release notes **UNSIGNED** with instructions to verify the SHA-256 checksum.

This is deliberate. A release pipeline that hard-fails without signing would block
shipping entirely during the validation wait, and one that silently skipped signing
would let an unsigned build be mistaken for a signed one. Announcing the mode in the
published notes is the honest middle path.

Both the executable and the installer are signed once enabled. The installer is a
separate binary and is the file users actually download and double-click, so it is the
one SmartScreen judges.

### Verification

Signatures are checked in CI with `Get-AuthenticodeSignature`, and the job fails if the
status is anything other than `Valid`. Users can run the same check locally:

```powershell
Get-AuthenticodeSignature .\wsl-traffic-monitor-<version>-setup.exe | Format-List
```

### Cost

Trusted Signing Basic bills roughly $10/month. It is live from 2026-08-09, before any
certificate can be issued — the account is billable independently of whether identity
validation has completed.
