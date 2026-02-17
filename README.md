# edera-check CLI

CLI tool to run checks and generate system reports before installing or using Edera.

---

## Usage

### General usage

```bash
sudo edera-check --help
```

### Run preinstall checks and generate system report

```bash
sudo edera-check preinstall
```

### Run official release via Docker

```bash
docker run \
  --pull always \
  --pid host \
  --privileged \
  us-central1-docker.pkg.dev/edera-protect/staging/edera-check:main preinstall
```

Podman et al should also work.

### Run locally from repo root via Docker

Recommended way to run locally and debug/validate, will use local copy of repo.

```bash
sh hack/debug/local.sh
```

---

## Example Output

```text
» sudo edera-check preinstall

Running Group System Checks - System requirement checks
✅ System Checks: Passed
    • Enough Memory: Passed
    • Enough Disk: Passed
Running Group PVH Checks - PVH capability checks
✅ PVH Checks: Passed
    • PVH Support: Passed
Running Group Kernel Checks - Kernel requirement checks
✅ Kernel Checks: Passed
    • Host Has Necessary Modules: Passed
    • Host Kernel Version Is Good: Passed
Running Group IOMMU Checks - IOMMU capability checks
✅ IOMMU Checks: Passed
    • IOMMU Support: Passed
Running Group System Info Recorder - Record system information for reporting purposes
✅ System Info Recorder: Passed
    • Record lspci -vvv: Passed
    • Record dmidecode: Passed
    • Record /proc/cpuinfo: Passed
    • Record /proc/cmdline: Passed
    • Record /boot/grub2/grub.cfg: Passed
    • Record boot/config-6.18.7-200.fc43.x86_64: Passed
✨ All Done! Report saved: /tmp/edera-preinstall-report-StolidWingnut-20260213-222635.tar.gz
```

Exit code is **non-zero** if any check/group fails or errors.

---

## Notes

Run

``` bash
edera-check preinstall --help
```

for a list of available configuration options and usage tweaks.

## Dev Notes

* [src/recorders](src/recorders) - Special category of checkers that capture host machine state and generate informational reports.
* [src/checkers](src/checkers) - All checkers that validate and actively interrogate various aspects of the host machine go here.
* [src/helpers](src/helpers) - Shared bits.

By default, everything runs inside the container, and any system tools or binaries or other bits needed by the checkers can be installed or baked inside the container. For checks that need to jump out and touch the host filesystem or state, we use the [src/helpers/host_executor.rs](src/helpers/host_executor.rs).
