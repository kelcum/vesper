# Security policy

Vesper runs on people's PCs and touches their Roblox installation, so we take
security reports seriously and respond to them first.

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x (latest) | Yes |
| Older | No — update to the latest release |

Only the newest release receives security fixes. Vesper updates itself after
you confirm, and every update is signed and verified before it installs.

## Reporting a vulnerability

**Please don't open a public issue for security problems.**

Report privately through GitHub's
[private vulnerability reporting](https://github.com/kelcum/vesper/security/advisories/new)
("Report a vulnerability" on the repository's Security tab).

Include, where you can:

- The Vesper version (Settings → Updates, or About).
- Your Windows version.
- What the problem is and what an attacker could do with it.
- Steps to reproduce, or a proof of concept.
- Whether it needs anything unusual (admin rights, a specific Roblox version, another program installed).

You can attach Vesper's diagnostics (Settings → Advanced → Copy diagnostics):
they are designed to contain no secrets and no Windows user name.

**Never send us passwords, Roblox cookies (`.ROBLOSECURITY`), Discord tokens,
or other secrets — not even your own.** Vesper never needs them, and we will
never ask for them. Anyone claiming to be from Vesper who asks for them is not.

## What to expect

- We acknowledge reports within 3 working days.
- We confirm the problem, agree on its severity with you, and keep you updated.
- Fixes ship as a new release. We credit you in the release notes unless you
  prefer not to be named.
- Please give us a reasonable time to fix the issue (normally up to 90 days)
  before disclosing it publicly.

## Scope

In scope: the Vesper application, its installer and updater, and the
xan.gripe/vesper site files that ship update manifests and downloads.

Out of scope: Roblox itself, Discord, Windows, and third-party launchers.
Report problems in those to their makers.

## What Vesper does and doesn't do

These are design commitments; a violation of any of them is a security bug:

- Vesper never reads Roblox cookies, auth tickets, passwords or tokens.
- Vesper never injects code into Roblox, never holds handles to the Roblox
  process beyond starting it, and never interferes with Roblox's anti-cheat.
- Vesper only removes or replaces files in Roblox's folder that it wrote
  itself, and backs up anything else before replacing it.
- Updates are only installed after the user confirms and the signature matches
  the key compiled into Vesper.
- Logs and diagnostics are redacted before they are written.
