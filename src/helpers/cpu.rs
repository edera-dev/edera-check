pub fn extract_cpu_vendor(cpuinfo: &str) -> String {
    for line in cpuinfo.lines() {
        if line.starts_with("vendor_id")
            && let Some(value) = line.split(':').nth(1)
        {
            return value.trim().to_string();
        }
    }
    String::from("Unknown")
}

pub fn extract_flags(cpuinfo: &str) -> String {
    for line in cpuinfo.lines() {
        if line.starts_with("flags")
            && let Some(value) = line.split(':').nth(1)
        {
            return value.trim().to_string();
        }
    }
    String::new()
}
