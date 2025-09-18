# Preflight CLI

Internal CLI to run **pre-deployment checks** before running workloads.  
Checks are organized into groups (e.g., `System Checks`, `Scripted Checks`) and controlled via environment variables.  

---

## Usage

Run inside Docker:

```bash
docker run \
  --pull always \
  --env RUST_LOG=debug \
  --env EDERA_PREFLIGHT_VERBOSE=true \
  --env EDERA_PREFLIGHT_TARGET_DIR='/host' \
  --env EDERA_PREFLIGHT_SKIP_GROUPS='ScriptedChecks;SystemChecks' \
  --env EDERA_PREFLIGHT_SCRIPTS_DIR=/scripts \
  --volume /:/host \
  --pid host \
  --net host \
  --privileged \
  us-central1-docker.pkg.dev/edera-protect/staging/protect-preflight:main
```

---

## Environment Variables

| Variable                      | Description                                            | Example                       |
| ----------------------------- | ------------------------------------------------------ | ----------------------------- |
| `RUST_LOG`                    | Log level (`error`, `warn`, `info`, `debug`, `trace`). | `debug`                       |
| `EDERA_PREFLIGHT_VERBOSE`     | Enable verbose output (`true`/`false`).                | `true`                        |
| `EDERA_PREFLIGHT_SKIP_GROUPS` | Semicolon-separated list of groups to skip.            | `SystemChecks;ScriptedChecks` |
| `EDERA_PREFLIGHT_SCRIPTS_DIR` | Directory containing custom shell-script checks.       | `/scripts`                    |
| `EDERA_PREFLIGHT_TARGET_DIR` | Directory to chroot to before running checks. Needed when running in a container.       | `/host`                    |
| `EDERA_PREFLIGHT_REPORT_DIR` | Directory to write a report to. Defaults to tmpdir       | `/tmp`                    |

---

## Example Output

```text
[2025-09-17T05:05:33Z INFO  preflight] Running Group [System Checks] - System requirement checks
[2025-09-17T05:05:33Z DEBUG preflight::system] total memory = 66617298944
[2025-09-17T05:05:33Z ERROR preflight] [System Checks] Errored: group errored
[2025-09-17T05:05:33Z INFO  preflight] [System Checks] Enough Memory: Passed
[2025-09-17T05:05:33Z ERROR preflight] [System Checks] Should Error: Errored: Pretending to error
[2025-09-17T05:05:33Z WARN  preflight] [System Checks] Should Fail: Failed: Pretending to fail
[2025-09-17T05:05:33Z INFO  preflight] Running Group [Scripted Checks] - Checks composed through small shell scripts
[2025-09-17T05:05:33Z ERROR preflight] [Scripted Checks] Errored: group errored
[2025-09-17T05:05:33Z INFO  preflight] [Scripted Checks] Should Pass: Passed
[2025-09-17T05:05:33Z WARN  preflight] [Scripted Checks] Should Fail: Failed: script returned Some(1)
[2025-09-17T05:05:33Z ERROR preflight] [Scripted Checks] /totally/fake/script: Errored: No such file or directory (os error 2)
Error: checks failed
```

* **INFO** → check passed or group started
* **WARN** → check failed
* **ERROR** → check errored or group errored

Exit code is **non-zero** if any check/group fails or errors.

---

## Notes

* Use `EDERA_PREFLIGHT_SKIP_GROUPS` to bypass slow or irrelevant checks.
* Script-based checks must be **executable** and located in `EDERA_PREFLIGHT_SCRIPTS_DIR`.

## Script Based Checks

Check the scripts [README.md](./scripts/README.md)
