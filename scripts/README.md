# Scripted Checks

This directory contains **script-based preflight checks**.  
Each script is executed as part of the `Scripted Checks` group when `EDERA_PREFLIGHT_SCRIPTS_DIR` points here.  

---

## Writing a Scripted Check

A scripted check is just an **executable shell script** that follows two simple rules:

1. **Set the check name** by printing a line starting with  

```
EDERA_PREFLIGHT_CHECK_NAME=<name>
```

This will be used in logs and results.

2. **Exit with the correct status code**:  
- `0` → check passed  
- non-zero → check failed (the return code is reported)  

---

## Example

```sh
#!/bin/sh

# Name shown in preflight output
echo "EDERA_PREFLIGHT_CHECK_NAME=Should Pass"

# Any other output is shown as context
echo "it does pass"

# Exit code decides the result
exit 0
```

---

## Notes

* Scripts must be **executable** (`chmod +x script.sh`).
* You can write in any language (bash, python, etc.) as long as it follows the rules above.
* Errors such as missing files or runtime exceptions will show up as **Errored** checks in the output.
