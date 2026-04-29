# edera-check CLI

[![Crates.io Version](https://img.shields.io/crates/v/edera-check)](https://crates.io/crates/edera-check)

A CLI tool to validate your system is ready to install or use Edera, and generate local readiness reports.

---

## Installation

### Run official release via signed Docker image (recommended, Docker must be installed)

```bash
docker run \
  --pull always \
  --pid host \
  --privileged \
  ghcr.io/edera-dev/edera-check:stable preinstall
```

`podman` et al should also work.

### Install static binary via `curl`

``` bash
curl -sL $(curl -s https://api.github.com/repos/edera-dev/edera-check/releases/latest | grep -oP '"browser_download_url": "\K.*linux-x86_64-musl[^"]*') -o edera-check
chmod +x edera-check
sudo ./edera-check
```

### Install via Cargo

``` bash
cargo install edera-check
sudo edera-check --help
```

## Usage

### General usage

```bash
sudo edera-check --help
```

### Run `preinstall` checks and generate system report

```bash
sudo edera-check preinstall
```

Run this on fresh system without Edera to validate it is ready for an Edera install.

### Run `postinstall` checks and generate system report

```bash
sudo edera-check postinstall
```

Run this on a system that has been booted into an Edera installation to validate it is functional and ready to use.

---

### Generate system report

```bash
sudo edera-check collect
```

Run this on a system that has been booted into an Edera installation to collect a system report, including logs and other useful troubleshooting information.

---

## Example Output

```text
» sudo edera-check preinstall

Collecting information about the current host as part of locally-generated preinstall report.
The information collected will remain on this host.
Running Group System Checks [Required] - System requirement checks
    • Enough Memory: Passed
    • Enough Disk: Passed
✅ System Checks: Passed
Running Group Kernel Checks [Required] - Kernel requirement checks
    • Host Has Necessary Modules: Passed
    • Host Kernel Version Is Good: Passed
✅ Kernel Checks: Passed
Running Group IOMMU Checks [Optional] - IOMMU capability checks
    • IOMMU Support: Passed
✅ IOMMU Checks: Passed
Running Group PVH Checks [Optional] - PVH capability checks
    • PVH Support: Passed
✅ PVH Checks: Passed
Running Group NUMA Checks [Advisory] - NUMA capability checks
    • IOMMU Support: Passed
✅ NUMA Checks: Passed
Running Group System Info Recorder [Advisory] - Record system information for reporting purposes
    • Captured lspci -vvv: Passed
    • Captured dmidecode: Passed
    • Captured /proc/cpuinfo: Passed
    • Captured /proc/cmdline: Passed
    • Captured /boot/grub2/grub.cfg: Passed
    • Captured boot/config-6.18.7-200.fc43.x86_64: Passed
    • Record current host kernel loaded modules: Passed
✅ System Info Recorder: Passed
✨ All Done! Report saved: ./edera-preinstall-report-StolidWingnut-20260218-003925.tar.gz
```

Exit code is **non-zero** if any check/group fails or errors.

---

## Notes

Run

``` bash
edera-check preinstall --help
edera-check postinstall --help
```

for a list of available configuration options, available check groups, and usage tweaks.

## Documentation

Where possible, each check or recorder has a `rustdoc` documentation comment
showing how to manually perform the equivalent check locally on your system,
if for some reason you are curious about how to run a specific check without using `edera-check` to do it.

You can see the example snippets for each documented check by [reading the crate docs](https://crates.io/crates/edera-check).

## Dev Notes

* [src/recorders](src/recorders) - Special category of checkers that capture host machine state and generate informational reports.
* [src/checkers](src/checkers) - All checkers that validate and actively interrogate various aspects of the host machine go here.
* [src/helpers](src/helpers) - Shared bits.

By default, everything runs inside the container, and any system tools or binaries or other bits needed by the checkers can be installed or baked inside the container. For checks that need to jump out and touch the host filesystem or state, we use container namespace escape: [src/helpers/host_executor.rs](src/helpers/host_executor.rs).

### Run locally from repo root via Docker

Recommended way to run locally and debug/validate, will use local copy of repo.

```bash
sh hack/debug/local.sh
```
