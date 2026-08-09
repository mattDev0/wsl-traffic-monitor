# ADR 0001: Code Signing Strategy for Windows Release Binaries

- **Status**: Superseded — Azure Trusted Signing is not available in our jurisdiction
- **Date**: 2026-08-01 (provisioned and withdrawn 2026-08-09)
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

## Outcome: Azure Trusted Signing rejected

Trusted Signing was provisioned on 2026-08-09 and **withdrawn the same day**. The
option was evaluated on cost and CI integration, but not on eligibility, and eligibility
is what disqualifies it.

### Why

Microsoft restricts Public Trust certificates by geography
([documentation](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart)):

> Public Trust certificates are available to organizations in the United States, Canada,
> the European Union, the United Kingdom, Australia, New Zealand, Japan, South Korea,
> Singapore, Switzerland, Norway, and Israel. Individual developers must be located in
> the United States or Canada. These geographic restrictions do not apply to Private
> Trust certificates.

This project is maintained from **Nigeria**, which appears on neither list. Incorporating
would not help, since the organisation list does not include it either.

Private Trust carries no geographic restriction but requires every user to install the
project's root certificate before the signature means anything. That is workable for a
managed fleet and useless for public distribution through GitHub Releases.

### What was provisioned and removed

A resource group, a Basic-SKU signing account, an app registration with OIDC federation,
and a role assignment were created, then deleted once the restriction was found. The
GitHub environment secrets were removed first: had they remained, the release workflow
would have detected credentials and attempted to sign against a deleted identity, turning
a working unsigned release into a failing one.

Cost incurred was one partial day of Basic SKU.

### Lesson for the next option

Eligibility should be confirmed before provisioning. The original evaluation compared
cost, CI integration and SmartScreen behaviour across three options and recorded
"requires Microsoft Azure active subscription and identity validation" as the only
constraint on Option 1. Whether the maintainer could legally obtain a certificate was
never asked.

## Revised options

### Option A: SignPath Foundation (preferred)

Free code-signing certificates for open-source projects, with GitHub Actions
integration. Eligibility is assessed on the project — public repository, OSS licence,
reproducible builds — rather than the maintainer's location, which is precisely the
constraint that ruled out Trusted Signing. GPL-3.0 with a public repository fits their
stated profile.

Unverified: whether Nigeria-based maintainers are accepted. That is the question to put
to them before any further work.

### Option B: Certum Open Source Code Signing

A Polish CA issuing to open-source developers internationally at roughly €90/year,
materially cheaper than DigiCert or Sectigo. Current versions require a hardware token
or cloud HSM; token delivery to Nigeria needs checking before purchase.

### Option C: Ship unsigned with checksum verification (current state)

What the project does today, and what many open-source Windows tools do. Users see a
SmartScreen warning once and click through. Every release publishes SHA-256 sums for
each artifact, and the release notes state plainly that the build is unsigned and how to
verify it.

## Decision

Adopt **Option C** for now and pursue **Option A** in parallel. An OV certificate at
£200-400/year is difficult to justify at this stage, and it earns SmartScreen reputation
gradually rather than immediately, so it would not deliver the step change that motivated
this ADR.

## Pipeline status

The signing integration remains in `.github/workflows/release.yml` and is dormant. It
checks whether signing credentials are present; absent, it emits a CI warning, skips
signing and verification, and labels the release notes UNSIGNED with instructions to
verify the checksum.

Whatever certificate is eventually obtained, the integration point is one step in that
workflow. The Trusted Signing action would be swapped for the relevant equivalent and the
credentials restored as environment secrets. Nothing else changes.
