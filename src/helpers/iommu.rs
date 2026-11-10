use std::{fs, path::Path};

use anyhow::Result;
use log::debug;

use crate::helpers::{
    cpu::{extract_flags, read_cpuinfo},
    host_executor::HostNamespaceExecutor,
};

const DMAR: &str = "/sys/firmware/acpi/tables/DMAR";
const IVRS: &str = "/sys/firmware/acpi/tables/IVRS";
const FADT: &str = "/sys/firmware/acpi/tables/FACP";

/// DMAR and IVRS both put their variable-length entries after a 48-byte header, with a u16
/// length at entry offset 2. DMAR entry types are u16, IVRS block types are u8.
fn has_entry(table: &[u8], wide_type: bool, wanted: &[u16]) -> bool {
    let mut offset = 48;
    while let Some(header) = table.get(offset..offset + 4) {
        let entry_type = if wide_type {
            u16::from_le_bytes([header[0], header[1]])
        } else {
            header[0].into()
        };
        if wanted.contains(&entry_type) {
            return true;
        }
        let length = u16::from_le_bytes([header[2], header[3]]);
        if length < 4 {
            return false;
        }
        offset += usize::from(length);
    }
    false
}

/// Returns why Xen will not enable the IOMMU on this host, or `None` if the conditions that
/// are visible before install all hold: a DMAR table with a DRHD unit and MSI allowed, or an
/// IVRS table with an IVHD block Xen accepts, plus CMPXCHG16B.
pub async fn hardware_iommu_problem(
    host_executor: &HostNamespaceExecutor,
) -> Result<Option<String>> {
    let cpuinfo = read_cpuinfo(host_executor).await?;
    let has_cx16 = extract_flags(&cpuinfo)
        .split_whitespace()
        .any(|f| f == "cx16");

    let problem = host_executor
        .spawn_in_host_ns(async move {
            if Path::new(DMAR).exists() {
                debug!("Found Intel IOMMU");
                let dmar = fs::read(DMAR)?;
                if dmar.get(36).is_none_or(|width| *width == 0) {
                    return Ok(Some("DMAR table has an invalid host address width"));
                }
                if !has_entry(&dmar, true, &[0]) {
                    return Ok(Some("DMAR table has no DRHD unit"));
                }
                // IAPC_BOOT_ARCH bit 3 forbids MSI, which Xen needs for VT-d.
                let fadt = fs::read(FADT)?;
                if fadt.get(109).is_some_and(|flags| flags & (1 << 3) != 0) {
                    return Ok(Some("firmware forbids MSI, which VT-d needs"));
                }
            } else if Path::new(IVRS).exists() {
                debug!("Found AMD IOMMU");
                let ivrs = fs::read(IVRS)?;
                // Xen only accepts type 0x11 IVHD blocks when IVinfo has EFRSup set.
                let efr_sup = ivrs.get(36).is_some_and(|info| info & 1 != 0);
                let wanted: &[u16] = if efr_sup { &[0x10, 0x11] } else { &[0x10] };
                if !has_entry(&ivrs, false, wanted) {
                    return Ok(Some("IVRS table has no usable IVHD block"));
                }
            } else {
                return Ok(Some("no hardware IOMMU detected"));
            }

            if !has_cx16 {
                return Ok(Some("CPU has no CMPXCHG16B support, which the IOMMU needs"));
            }
            anyhow::Ok(None)
        })
        .await??;
    Ok(problem.map(String::from))
}

#[cfg(test)]
mod tests {
    use super::has_entry;

    fn table(entries: &[&[u8]]) -> Vec<u8> {
        let mut table = vec![0u8; 48];
        for entry in entries {
            table.extend_from_slice(entry);
        }
        table
    }

    #[test]
    fn finds_drhd_after_other_dmar_entries() {
        // RMRR (type 1, 8 bytes), then DRHD (type 0, 16 bytes).
        let rmrr = [1, 0, 8, 0, 0, 0, 0, 0];
        let drhd = [0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(has_entry(&table(&[&rmrr, &drhd]), true, &[0]));
        assert!(!has_entry(&table(&[&rmrr]), true, &[0]));
        assert!(!has_entry(&table(&[]), true, &[0]));
    }

    #[test]
    fn ivhd_type_must_be_one_xen_accepts() {
        // IVMD (type 0x20, 8 bytes), then IVHD type 0x11 (8 bytes).
        let ivmd = [0x20, 0, 8, 0, 0, 0, 0, 0];
        let ivhd_11 = [0x11, 0, 8, 0, 0, 0, 0, 0];
        let ivrs = table(&[&ivmd, &ivhd_11]);
        assert!(has_entry(&ivrs, false, &[0x10, 0x11]));
        assert!(!has_entry(&ivrs, false, &[0x10]));
    }

    #[test]
    fn stops_on_truncated_or_zero_length_entry() {
        assert!(!has_entry(
            &table(&[&[1, 0, 0, 0], &[0, 0, 16, 0]]),
            true,
            &[0]
        ));
        assert!(!has_entry(&table(&[&[1, 0]]), true, &[0]));
    }
}
