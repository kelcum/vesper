# Code signing policy

Vesper's Windows releases are built by GitHub Actions from the public
repository, from the tagged commit, and nothing else. Free code signing
provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org), once the application is approved.
Until then, Windows releases are unsigned and Windows Security may warn about
them.

## Team

- Committers and reviewers: [kelcum](https://github.com/kelcum)
- Approvers: [kelcum](https://github.com/kelcum)

Every release is a pull request merged by the maintainer, then a version tag
that the release workflow builds. Changes from anyone else go through a pull
request reviewed by the maintainer before they are merged.

## Privacy

Vesper's [privacy page](https://xan.gripe/vesper/privacy/) lists every network
request it makes. There is no telemetry, no account and no identifier. The
requests that happen without being asked are anonymous version and update
checks, each with a setting to turn it off.

## Releases

Signed releases carry "Vesper" as the product name and the release version as
the product version. The installer never changes system settings and uninstalls
cleanly from Windows' "Installed apps", putting Roblox's files back as it found
them.
