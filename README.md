# Preflight CLI

Internal CLI to run **pre-deployment checks** before running workloads.  
Checks are organized into groups (e.g., `PVH Checks`, `Kernel Checks`) and controlled via environment variables.

---

## Usage

### Run official release via Docker

```bash
docker run \
  --pull always \
  --env RUST_LOG=debug \
  --env EDERA_PREFLIGHT_VERBOSE=true \
  --env EDERA_PREFLIGHT_SKIP_GROUPS='PVHChecks;KernelChecks' \
  --pid host \
  --privileged \
  us-central1-docker.pkg.dev/edera-protect/staging/protect-preflight:main
```

Podman should also work.

### Run locally from repo root via Docker

Recommended way to run locally and debug/validate, will use local copy of repo.

```bash
sh hack/debug/local.sh
```

---

## Environment Variables

| Variable                      | Description                                            | Example                       |
| ----------------------------- | ------------------------------------------------------ | ----------------------------- |
| `RUST_LOG`                    | Log level (`error`, `warn`, `info`, `debug`, `trace`). | `debug`                       |
| `EDERA_PREFLIGHT_VERBOSE`     | Enable verbose output (`true`/`false`).                | `true`                        |
| `EDERA_PREFLIGHT_SKIP_GROUPS` | Semicolon-separated list of groups to skip.            | `PVHChecks;KernelChecks` |
| `EDERA_PREFLIGHT_REPORT_DIR` | Directory to write a report to. Defaults to tmpdir       | `/tmp`                    |

---

## Example Output

```text
[2026-02-13T00:28:35Z INFO  preflight] Writing all files to /tmp/protect-preflight-bundle-20260213-002835
[2026-02-13T00:28:35Z INFO  preflight] Running Group [System Checks] - System requirement checks
[2026-02-13T00:28:35Z DEBUG preflight::checkers::system] Enough space on disk mounted at / - 617760940032
[2026-02-13T00:28:35Z DEBUG preflight::checkers::system] Not enough space on disk mounted at /etc/resolv.conf - 9729925120
[2026-02-13T00:28:35Z DEBUG preflight::checkers::system] Enough space on disk mounted at /etc/hostname - 617760940032
[2026-02-13T00:28:35Z DEBUG preflight::checkers::system] Enough space on disk mounted at /etc/hosts - 617760940032[2026-02-13T00:28:35Z DEBUG preflight::checkers::system] total memory = 28762972160
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Checks] Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Checks] Enough Memory: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Checks] Enough Disk: Passed
[2026-02-13T00:28:35Z INFO  preflight] Running Group [PVH Checks] - PVH capability checks
[2026-02-13T00:28:35Z DEBUG preflight::checkers::pvh] VM_CR=0x0 (svmdis=0)
[2026-02-13T00:28:35Z DEBUG preflight::checkers::pvh] EFER=0x200d01 (SVME=0)
[2026-02-13T00:28:35Z DEBUG preflight::checkers::pvh] AMD-V supported but unavailable under hypervisor
[2026-02-13T00:28:35Z DEBUG preflight::checkers::pvh] Virtualization is enabled or can be enabled
[2026-02-13T00:28:35Z DEBUG preflight::helpers] [PVH Checks] Skipped
[2026-02-13T00:28:35Z INFO  preflight::helpers] [PVH Checks] PVH Support: Passed
[2026-02-13T00:28:35Z INFO  preflight] Running Group [Kernel Checks] - Kernel requirement checks
[2026-02-13T00:28:35Z DEBUG preflight::checkers::kernel] module msr
[2026-02-13T00:28:35Z DEBUG preflight::checkers::kernel] module nf_tables
[2026-02-13T00:28:35Z INFO  preflight::helpers] [Kernel Checks] Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [Kernel Checks] Host Has Necessary Modules: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [Kernel Checks] Host Kernel Version Is Good: Passed
[2026-02-13T00:28:35Z INFO  preflight] Running Group [System Info Recorder] - System requirement and status checks - records for informational purposes
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record lspci -vvv: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record dmidecode: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record /proc/cpuinfo: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record /proc/cmdline: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record /boot/grub2/grub.cfg: Passed
[2026-02-13T00:28:35Z INFO  preflight::helpers] [System Info Recorder] Record boot/config-6.18.6: Passed
[2026-02-13T00:28:35Z DEBUG preflight] Read 84023 bytes of tar
[2026-02-13T00:28:35Z INFO  preflight] Wrote to: /tmp/protect-preflight-bundle-20260213-002835.tar.gz
```

* **INFO** → check passed or group started
* **WARN** → check failed
* **ERROR** → check errored or group errored

Exit code is **non-zero** if any check/group fails or errors.

---

## Notes

* Use `EDERA_PREFLIGHT_SKIP_GROUPS` to bypass slow or irrelevant checks.

## Dev Notes

* [src/recorders](src/recorders) - Special category of checkers that capture host machine state and generate informational reports.
* [src/checkers](src/checkers) - All checkers that validate and actively interrogate various aspects of the host machine go here.
* [src/helpers](src/helpers) - Shared bits.

By default, everything runs inside the container, and any system tools or binaries or other bits needed by the checkers can be installed or baked inside the container. For checks that need to jump out and touch the host filesystem or state, we use the [src/helpers/host_executor.rs](src/helpers/host_executor.rs).
