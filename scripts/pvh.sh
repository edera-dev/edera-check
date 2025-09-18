#!/bin/sh
set -eu

echo "EDERA_PREFLIGHT_CHECK_NAME=Can PVH Be Enabled"

# prerequisites: msr-tools (rdmsr), and the msr kernel module
modprobe msr 2>/dev/null || true

err() {
  echo "$@" >&2
}

do_rdmsr() {
  MSR="$1"
  if [ ! -e /dev/cpu/0/msr ]; then
    err "rdmsr ${MSR}: /dev/cpu/0/msr doesn't exist, load 'msr' kernel module"
    return 1
  fi

  if command -v rdmsr >/dev/null; then
    rdmsr -p0 -d "$MSR" 2>/dev/null || echo 0
  elif command -v dd >/dev/null && command -v od >/dev/null; then
    (dd if=/dev/cpu/0/msr bs=1 skip=$(($MSR)) count=8 status=none 2>/dev/null | od -An -tu8 -N8) || echo 0
  else
    err "rdmsr ${MSR}: need either 'rdmsr' or 'dd' and 'od' commands and none were found"
    return 1
  fi
}

cpu_vendor=$(awk -F: '/vendor_id/{print $2; exit}' /proc/cpuinfo | xargs)
flags=$(awk -F: '/^flags/{print $2; exit}' /proc/cpuinfo)

case "$cpu_vendor" in
"GenuineIntel")
  echo "$flags" | grep -qw vmx && cap=yes || cap=no

  if do_rdmsr "0x3a" >/dev/null; then
    val=$(do_rdmsr 0x3a 2>/dev/null || echo 0)
    lock=$(((val >> 0) & 1))
    vmx_outside_smx=$(((val >> 2) & 1))
    bios_enabled=no
    if [ $lock -eq 1 ] && [ $vmx_outside_smx -eq 1 ]; then
      bios_enabled=yes
    fi
    printf "Intel VT-x capability: %s\n" "$cap"
    printf "IA32_FEATURE_CONTROL (0x3A): 0x%x (lock=%d, vmx_outside_smx=%d)\n" "$val" "$lock" "$vmx_outside_smx"
    printf "BIOS permits VT-x: %s\n" "$bios_enabled"
  else
    printf "Intel VT-x capability: %s (install msr-tools to verify MSR 0x3A)\n" "$cap"
  fi
  ;;

"AuthenticAMD")
  echo "$flags" | grep -qw svm && cap=yes || cap=no
  if do_rdmsr "0xC0010114" >/dev/null; then
    vmcr=$(do_rdmsr 0xC0010114 2>/dev/null || echo 0)
    lock=$(((vmcr >> 3) & 1))   # VM_CR.LOCK
    svmdis=$(((vmcr >> 4) & 1)) # VM_CR.SVMDIS (1 = disabled by BIOS/firmware)
    bios_enabled=$([ $svmdis -eq 0 ] && echo yes || echo no)

    efer=$(do_rdmsr 0xC0000080 2>/dev/null || echo 0)
    svme=$(((efer >> 12) & 1)) # EFER.SVME (runtime: 1 if OS/hypervisor enabled SVM)

    printf "AMD SVM capability: %s\n" "$cap"
    printf "VM_CR (0xC0010114): 0x%x (lock=%d, svmdis=%d)\n" "$vmcr" "$lock" "$svmdis"
    printf "BIOS permits SVM: %s\n" "$bios_enabled"
    printf "EFER (0xC0000080): 0x%x (SVME=%d)\n" "$efer" "$svme"
  else
    printf "AMD SVM capability: %s (install msr-tools to verify VM_CR/EFER)\n" "$cap"
  fi
  ;;

*)
  echo "Unknown CPU vendor: $cpu_vendor"
  ;;
esac

# vim: set ts=2 sts=2 sw=2 et:
