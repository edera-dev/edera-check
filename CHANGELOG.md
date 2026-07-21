# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.22](https://github.com/edera-dev/edera-check/compare/v0.2.21...v0.2.22) - 2026-07-21

### Added

- Add KVM based checks

### Other

- Update images digests ([#145](https://github.com/edera-dev/edera-check/pull/145))
- Bump the cargo-updates group across 1 directory with 7 updates ([#157](https://github.com/edera-dev/edera-check/pull/157))

## [0.2.21](https://github.com/edera-dev/edera-check/compare/v0.2.20...v0.2.21) - 2026-06-05

### Other

- Update images digests ([#137](https://github.com/edera-dev/edera-check/pull/137))
- Bump the cargo-updates group across 1 directory with 3 updates ([#142](https://github.com/edera-dev/edera-check/pull/142))

## [0.2.20](https://github.com/edera-dev/edera-check/compare/v0.2.19...v0.2.20) - 2026-05-20

### Other

- Use journalctl export format ([#133](https://github.com/edera-dev/edera-check/pull/133))
- Capture links/route table/neighbor table/nftables contents for v4/v6 ([#125](https://github.com/edera-dev/edera-check/pull/125))
- Bump the cargo-updates group across 1 directory with 2 updates ([#131](https://github.com/edera-dev/edera-check/pull/131))
- Update images digests ([#127](https://github.com/edera-dev/edera-check/pull/127))

## [0.2.19](https://github.com/edera-dev/edera-check/compare/v0.2.18...v0.2.19) - 2026-04-30

### Other

- Docker needs a TTY for le pretty colours ([#126](https://github.com/edera-dev/edera-check/pull/126))
- Update images digests ([#121](https://github.com/edera-dev/edera-check/pull/121))
- Bump the cargo-updates group across 1 directory with 2 updates ([#120](https://github.com/edera-dev/edera-check/pull/120))
- Add `collect` subcommand that only runs postinstall debug collection ([#112](https://github.com/edera-dev/edera-check/pull/112))
- Share sysinfo recorders that are common across both preinstall and postinstall (i.e. many of them) ([#124](https://github.com/edera-dev/edera-check/pull/124))
- Catch Ubuntu's `snap`-based Docker runtime ([#118](https://github.com/edera-dev/edera-check/pull/118))
- Bump the cargo-updates group with 2 updates ([#114](https://github.com/edera-dev/edera-check/pull/114))

## [0.2.18](https://github.com/edera-dev/edera-check/compare/v0.2.17...v0.2.18) - 2026-04-17

### Other

- Capture oxenstored logs ([#111](https://github.com/edera-dev/edera-check/pull/111))

## [0.2.17](https://github.com/edera-dev/edera-check/compare/v0.2.16...v0.2.17) - 2026-04-17

### Other

- Update images digests ([#98](https://github.com/edera-dev/edera-check/pull/98))
- Use `dmidecode` crate to avoid hostbin ([#91](https://github.com/edera-dev/edera-check/pull/91))
- Use friendly units for memcheck ([#109](https://github.com/edera-dev/edera-check/pull/109))
- Add boot log ([#106](https://github.com/edera-dev/edera-check/pull/106))
- Postinstall disk advisory check ([#107](https://github.com/edera-dev/edera-check/pull/107))

## [0.2.16](https://github.com/edera-dev/edera-check/compare/v0.2.15...v0.2.16) - 2026-04-01

### Other

- `preinstall`, like `postinstall` should also check loadable (versus just loaded or builtin) ([#95](https://github.com/edera-dev/edera-check/pull/95))

## [0.2.15](https://github.com/edera-dev/edera-check/compare/v0.2.14...v0.2.15) - 2026-04-01

### Other

- postinstall as well ([#94](https://github.com/edera-dev/edera-check/pull/94))
- Unlike `/etc/hostname`, this should always be present ([#92](https://github.com/edera-dev/edera-check/pull/92))

## [0.2.14](https://github.com/edera-dev/edera-check/compare/v0.2.13...v0.2.14) - 2026-04-01

### Other

- Update images digests ([#83](https://github.com/edera-dev/edera-check/pull/83))
- Bump the cargo-updates group across 1 directory with 2 updates ([#85](https://github.com/edera-dev/edera-check/pull/85))
- Remove `pub` from methods that don't need it ([#89](https://github.com/edera-dev/edera-check/pull/89))
- Make disk checks smarter ([#87](https://github.com/edera-dev/edera-check/pull/87))

## [0.2.13](https://github.com/edera-dev/edera-check/compare/v0.2.12...v0.2.13) - 2026-03-26

### Other

- Fix `nft` check incorrectly not running in the host context ([#78](https://github.com/edera-dev/edera-check/pull/78))

## [0.2.12](https://github.com/edera-dev/edera-check/compare/v0.2.11...v0.2.12) - 2026-03-26

### Other

- Add hv-debug-info ([#76](https://github.com/edera-dev/edera-check/pull/76))

## [0.2.11](https://github.com/edera-dev/edera-check/compare/v0.2.10...v0.2.11) - 2026-03-24

### Other

- Also get kubelet logs ([#75](https://github.com/edera-dev/edera-check/pull/75))
- Capture daemon logs in bundle ([#73](https://github.com/edera-dev/edera-check/pull/73))

## [0.2.10](https://github.com/edera-dev/edera-check/compare/v0.2.9...v0.2.10) - 2026-03-23

### Other

- Bump the cargo-updates group across 1 directory with 2 updates ([#66](https://github.com/edera-dev/edera-check/pull/66))
- Forgot to wrap these in hostexec closures ([#70](https://github.com/edera-dev/edera-check/pull/70))

## [0.2.9](https://github.com/edera-dev/edera-check/compare/v0.2.8...v0.2.9) - 2026-03-23

### Other

- Explicitly check for remaining required hostbins ([#68](https://github.com/edera-dev/edera-check/pull/68))

## [0.2.8](https://github.com/edera-dev/edera-check/compare/v0.2.7...v0.2.8) - 2026-03-18

### Other

- Check for presence of `nft` binary in $PATH ([#64](https://github.com/edera-dev/edera-check/pull/64))

## [0.2.7](https://github.com/edera-dev/edera-check/compare/v0.2.6...v0.2.7) - 2026-03-02

### Other

- Add `rustdoc` autogen docs for checkers/recorders ([#53](https://github.com/edera-dev/edera-check/pull/53))
- Update images digests ([#48](https://github.com/edera-dev/edera-check/pull/48))

## [0.2.6](https://github.com/edera-dev/edera-check/compare/v0.2.5...v0.2.6) - 2026-02-20

### Other

- Fix typo in README ([#47](https://github.com/edera-dev/edera-check/pull/47))
- Actually wrap the badge in a link ([#45](https://github.com/edera-dev/edera-check/pull/45))
- Cargo badge ([#43](https://github.com/edera-dev/edera-check/pull/43))

## [0.2.5](https://github.com/edera-dev/edera-check/compare/v0.2.4...v0.2.5) - 2026-02-19

### Other

- Fixup readme and native runner CI ([#41](https://github.com/edera-dev/edera-check/pull/41))

## [0.2.4](https://github.com/edera-dev/edera-check/compare/v0.2.3...v0.2.4) - 2026-02-19

### Other

- Attempt multiarch ([#37](https://github.com/edera-dev/edera-check/pull/37))
- Fixup readme for CURL install, fix arm build ([#35](https://github.com/edera-dev/edera-check/pull/35))

## [0.2.3](https://github.com/edera-dev/edera-check/compare/v0.2.2...v0.2.3) - 2026-02-19

### Other

- Bump deps ([#33](https://github.com/edera-dev/edera-check/pull/33))

## [0.2.2](https://github.com/edera-dev/edera-check/compare/v0.2.1...v0.2.2) - 2026-02-19

### Other

- Fix action name ([#31](https://github.com/edera-dev/edera-check/pull/31))

## [0.2.1](https://github.com/edera-dev/edera-check/compare/v0.2.0...v0.2.1) - 2026-02-19

### Other

- Trigger release-plz ([#29](https://github.com/edera-dev/edera-check/pull/29))

## [0.2.0](https://github.com/edera-dev/edera-check/releases/tag/v0.2.0) - 2026-02-18

### Added

- initial add of edera-check utility

### Fixed


### Other

