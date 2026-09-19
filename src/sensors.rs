use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviceKind {
    Cpu,
    Gpu,
    Nvme,
    Memory,
    Motherboard,
    Battery,
    Other,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceKind::Cpu => write!(f, "CPU"),
            DeviceKind::Gpu => write!(f, "GPU"),
            DeviceKind::Nvme => write!(f, "NVMe"),
            DeviceKind::Memory => write!(f, "RAM"),
            DeviceKind::Motherboard => write!(f, "Mobo"),
            DeviceKind::Battery => write!(f, "BAT"),
            DeviceKind::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensorDescriptor {
    pub id: String,
    pub kind: DeviceKind,
    pub device_name: String,
    /// Vendor model string for NVMe controllers (e.g. "Micron MTFDKCD512QGN-1BN1AABLA").
    pub model: Option<String>,
    pub label: String,
    pub input_path: PathBuf,
    pub crit_temp: Option<f64>,
    pub max_temp: Option<f64>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SensorReading {
    pub id: String,
    pub kind: DeviceKind,
    pub device_name: String,
    /// Vendor model string for NVMe controllers, when discoverable via sysfs.
    pub model: Option<String>,
    pub label: String,
    pub temp: f64,
    pub crit_temp: Option<f64>,
    pub max_temp: Option<f64>,
}

impl SensorReading {
    #[allow(dead_code)]
    pub fn display_label(&self) -> String {
        if self.label.is_empty() || self.label == "Unknown" {
            format!("[{}] {}", self.kind, self.device_name)
        } else {
            format!("[{}] {}", self.kind, self.label)
        }
    }

    pub fn is_cpu(&self) -> bool {
        self.kind == DeviceKind::Cpu
    }

    pub fn is_gpu(&self) -> bool {
        self.kind == DeviceKind::Gpu
    }

    pub fn is_nvme(&self) -> bool {
        self.kind == DeviceKind::Nvme
    }

    /// Human-facing device name, resolved from sysfs/DMI per device kind
    /// (e.g. "AMD Ryzen AI 7 350 w/ Radeon 860M", "Micron MTFDKCD512QGN-1BN1AABLA",
    /// "BYD L24B3PK2"). Falls back to the kernel node name ("nvme0").
    pub fn display_name(&self) -> String {
        if let Some(model) = self.model.as_deref().filter(|m| !m.is_empty()) {
            return model.to_string();
        }
        self.device_name.clone()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuFreq {
    pub max_ghz: f64,
    pub avg_ghz: f64,
}

pub struct SensorRegistry {
    descriptors: Vec<SensorDescriptor>,
}

impl SensorRegistry {
    pub fn scan() -> Self {
        let descriptors = discover_sensors();
        Self { descriptors }
    }

    #[allow(dead_code)]
    pub fn descriptors(&self) -> &[SensorDescriptor] {
        &self.descriptors
    }

    pub fn read_all(&self) -> Vec<SensorReading> {
        read_descriptors(&self.descriptors)
    }

    #[allow(dead_code)]
    pub fn refresh(&mut self) {
        self.descriptors = discover_sensors();
    }
}

fn classify_device(name: &str, path: &Path) -> DeviceKind {
    let lower_name = name.to_lowercase();
    if lower_name.contains("coretemp")
        || lower_name.contains("k10temp")
        || lower_name.contains("zenpower")
        || lower_name.contains("cpu_thermal")
        || lower_name.contains("cpu")
    {
        DeviceKind::Cpu
    } else if lower_name.contains("amdgpu")
        || lower_name.contains("nvidia")
        || lower_name.contains("nouveau")
        || lower_name.contains("i915")
        || lower_name.contains("xe")
    {
        DeviceKind::Gpu
    } else if lower_name.contains("nvme") {
        DeviceKind::Nvme
    } else if lower_name.contains("spd5118")
        || lower_name.contains("ee1004")
        || lower_name.contains("dimm")
    {
        DeviceKind::Memory
    } else if lower_name.contains("acpitz")
        || lower_name.contains("nct6775")
        || lower_name.contains("it87")
        || lower_name.contains("asus")
        || lower_name.contains("thinkpad")
    {
        DeviceKind::Motherboard
    } else if lower_name.contains("bat") || lower_name.contains("battery") {
        DeviceKind::Battery
    } else {
        // Check symlink destination for hints
        if let Ok(target) = fs::read_link(path) {
            let target_str = target.to_string_lossy().to_lowercase();
            if target_str.contains("nvme") {
                return DeviceKind::Nvme;
            }
            if target_str.contains("amdgpu") || target_str.contains("nvidia") || target_str.contains("drm") {
                return DeviceKind::Gpu;
            }
        }
        DeviceKind::Other
    }
}

/// Resolve the vendor model of an NVMe controller (e.g. "nvme0") from sysfs.
/// This is the native equivalent of `lsblk -o MODEL` for a single device.
fn resolve_nvme_model(device_name: &str) -> Option<String> {
    if !device_name.starts_with("nvme") {
        return None;
    }
    let candidates = [
        format!("/sys/class/nvme/{}/model", device_name),
        format!("/sys/block/{}n1/device/model", device_name),
    ];
    for candidate in candidates {
        if let Ok(content) = fs::read_to_string(&candidate) {
            let model = content.trim();
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }
    }
    None
}

/// Best-effort human-readable device name for any sensor kind. Returns `None`
/// when the kernel exposes no useful identifier, letting callers fall back to
/// the device's kernel node name.
fn resolve_device_model(kind: DeviceKind, hwmon_path: &Path, device_name: &str) -> Option<String> {
    match kind {
        DeviceKind::Cpu => read_cpu_model(),
        DeviceKind::Gpu => resolve_gpu_model(hwmon_path),
        DeviceKind::Nvme => resolve_nvme_model(device_name),
        DeviceKind::Memory => resolve_dimm_name(hwmon_path),
        DeviceKind::Motherboard => resolve_dmi_name(),
        DeviceKind::Battery => resolve_power_supply_name(device_name),
        _ => None,
    }
}

/// Read a sysfs file and return its trimmed contents, or `None` when missing/empty.
fn read_trim(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read the CPU marketing name from `/proc/cpuinfo` (e.g. "AMD Ryzen AI 7 350").
fn read_cpu_model() -> Option<String> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            if let Some((_, value)) = rest.split_once(':') {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn resolve_gpu_model(hwmon_path: &Path) -> Option<String> {
    // NVIDIA's proprietary driver publishes the board name under /proc.
    if let Ok(entries) = fs::read_dir("/proc/driver/nvidia/gpus") {
        for entry in entries.flatten() {
            if let Ok(info) = fs::read_to_string(entry.path().join("information")) {
                for line in info.lines() {
                    if let Some(value) = line.strip_prefix("Model:") {
                        let value = value.trim();
                        if !value.is_empty() {
                            return Some(value.to_string());
                        }
                    }
                }
            }
        }
    }

    // Some AMD drivers expose the marketing name in DRM sysfs.
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Whole cards only ("card0"), not connectors ("card0-DP-1").
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            if let Some(product) = read_trim(&entry.path().join("device/product_name")) {
                return Some(product);
            }
        }
    }

    // Fall back to the PCI vendor of the controller backing this hwmon.
    let vendor = fs::read_to_string(hwmon_path.join("device/vendor")).ok()?;
    match vendor.trim() {
        "0x1002" => {
            // AMD APUs usually name the integrated Radeon in the CPU model string.
            // Only trust that when there is a single GPU, so a discrete card on an
            // APU system isn't mislabelled with the integrated name.
            if count_drm_cards() <= 1 {
                let cpu = read_cpu_model().unwrap_or_default();
                if let Some(idx) = cpu.find("Radeon") {
                    return Some(cpu[idx..].trim().to_string());
                }
            }
            Some("AMD GPU".to_string())
        }
        "0x10de" => Some("NVIDIA GPU".to_string()),
        "0x8086" => Some("Intel GPU".to_string()),
        _ => None,
    }
}

/// Count whole DRM cards ("card0"), ignoring connectors ("card0-DP-1").
fn count_drm_cards() -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("card") && !name.contains('-') {
                count += 1;
            }
        }
    }
    count
}

/// SPD5118 sensors live at `.../i2c-<bus>/<bus>-<addr>`. JEDEC SPD addresses
/// span 0x50..=0x57, so map them onto slot numbers: 0x50 -> "DIMM 0".
fn resolve_dimm_name(hwmon_path: &Path) -> Option<String> {
    let real = fs::canonicalize(hwmon_path.join("device")).ok()?;
    let leaf = real.file_name()?.to_string_lossy();
    let addr_str = leaf.rsplit('-').next()?;
    let addr = u32::from_str_radix(addr_str, 16).ok()?;
    if (0x50..=0x57).contains(&addr) {
        Some(format!("DIMM {}", addr - 0x50))
    } else {
        None
    }
}

/// Machine/board identity from DMI (e.g. "LENOVO 83NJ").
fn resolve_dmi_name() -> Option<String> {
    let vendor = read_trim(Path::new("/sys/class/dmi/id/sys_vendor"));
    let product = read_trim(Path::new("/sys/class/dmi/id/product_name"));
    match (vendor, product) {
        (Some(vendor), Some(product)) => {
            if product.to_lowercase().contains(&vendor.to_lowercase()) {
                Some(product)
            } else {
                Some(format!("{} {}", vendor, product))
            }
        }
        (Some(vendor), None) => Some(vendor),
        (None, Some(product)) => Some(product),
        (None, None) => None,
    }
}

/// Battery/AC name from the power-supply class (e.g. "BYD L24B3PK2").
fn resolve_power_supply_name(device_name: &str) -> Option<String> {
    let base = PathBuf::from("/sys/class/power_supply").join(device_name);
    if !base.is_dir() {
        return None;
    }
    let manufacturer = read_trim(&base.join("manufacturer"));
    let model = read_trim(&base.join("model_name"));
    match (manufacturer, model) {
        (Some(manufacturer), Some(model)) => Some(format!("{} {}", manufacturer, model)),
        (Some(manufacturer), None) => Some(manufacturer),
        (None, Some(model)) => Some(model),
        (None, None) => match read_trim(&base.join("type")).as_deref() {
            // AC adapters frequently expose no model at all.
            Some("Mains") => Some("AC Adapter".to_string()),
            Some("USB") => Some("USB Power".to_string()),
            _ => None,
        },
    }
}

fn resolve_device_name(hwmon_path: &Path, base_name: &str) -> String {
    // For NVMe devices, extract the specific device node like "nvme0" or "nvme1"
    let device_symlink = hwmon_path.join("device");
    if let Ok(target) = fs::read_link(&device_symlink) {
        if let Some(fname) = target.file_name() {
            let name_str = fname.to_string_lossy();
            if name_str.starts_with("nvme") {
                return name_str.to_string();
            }
        }
    }
    // Also check parent directory (e.g. .../nvme/nvme0/hwmon0)
    if let Some(parent) = hwmon_path.parent() {
        if let Some(parent_name) = parent.file_name() {
            let name_str = parent_name.to_string_lossy();
            if name_str.starts_with("nvme") {
                return name_str.to_string();
            }
        }
    }
    base_name.to_string()
}

fn read_temp_file(path: &Path) -> Option<f64> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if let Ok(millidegrees) = trimmed.parse::<f64>() {
        // hwmon reports temperatures in millidegrees Celsius
        Some(millidegrees / 1000.0)
    } else {
        None
    }
}

pub fn discover_sensors() -> Vec<SensorDescriptor> {
    let mut descriptors = Vec::new();
    let hwmon_dir = Path::new("/sys/class/hwmon");

    let entries = match fs::read_dir(hwmon_dir) {
        Ok(e) => e,
        Err(_) => return descriptors,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let base_name = match fs::read_to_string(path.join("name")) {
            Ok(n) => n.trim().to_string(),
            Err(_) => {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }
        };

        let kind = classify_device(&base_name, &path);
        let device_name = resolve_device_name(&path, &base_name);
        let model = resolve_device_model(kind, &path, &device_name);

        let dir_entries = match fs::read_dir(&path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for sub_entry in dir_entries.filter_map(Result::ok) {
            let file_name = sub_entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with("temp") && file_name.ends_with("_input") {
                let prefix = &file_name[..file_name.len() - 6]; // e.g. "temp1"

                // Look for label
                let label_file = path.join(format!("{}_label", prefix));
                let label = fs::read_to_string(&label_file)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| {
                        if kind == DeviceKind::Cpu && prefix == "temp1" {
                            "Tctl/Package".to_string()
                        } else {
                            format!("{}:{}", device_name, prefix)
                        }
                    });

                // Read optional crit and max thresholds (filter out bogus hardware values > 200°C)
                let crit_temp = read_temp_file(&path.join(format!("{}_crit", prefix)))
                    .filter(|t| *t > -40.0 && *t < 200.0);
                let max_temp = read_temp_file(&path.join(format!("{}_max", prefix)))
                    .filter(|t| *t > -40.0 && *t < 200.0);

                let id = format!("{}_{}", device_name, prefix);

                descriptors.push(SensorDescriptor {
                    id,
                    kind,
                    device_name: device_name.clone(),
                    model: model.clone(),
                    label,
                    input_path: sub_entry.path(),
                    crit_temp,
                    max_temp,
                });
            }
        }
    }

    // Sort sensors: CPUs first, GPUs second, NVMe third, RAM, Mobo, Battery, Other
    descriptors.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.device_name.cmp(&b.device_name))
            .then_with(|| a.label.cmp(&b.label))
    });

    descriptors
}

pub fn read_descriptors(descriptors: &[SensorDescriptor]) -> Vec<SensorReading> {
    let mut readings = Vec::with_capacity(descriptors.len());

    for desc in descriptors {
        if let Some(temp) = read_temp_file(&desc.input_path) {
            if temp > -50.0 && temp < 200.0 {
                readings.push(SensorReading {
                    id: desc.id.clone(),
                    kind: desc.kind,
                    device_name: desc.device_name.clone(),
                    model: desc.model.clone(),
                    label: desc.label.clone(),
                    temp,
                    crit_temp: desc.crit_temp,
                    max_temp: desc.max_temp,
                });
            }
        }
    }

    readings
}

pub fn search_sensors() -> std::io::Result<Vec<SensorReading>> {
    let descriptors = discover_sensors();
    Ok(read_descriptors(&descriptors))
}

/// Squash multi-channel sensors (e.g. NVMe) per physical device unless full_devices_sensors is requested.
pub fn squash_sensors(readings: &[SensorReading], full_devices_sensors: bool) -> Vec<SensorReading> {
    if full_devices_sensors {
        // Prefix each named device's channels with its resolved name
        return readings
            .iter()
            .map(|r| {
                let mut cloned = r.clone();
                // Tag every device that has a resolved name (NVMe always, so its
                // kernel node stays visible when no model is exposed).
                if cloned.kind == DeviceKind::Nvme || cloned.model.is_some() {
                    let display = cloned.display_name();
                    if !cloned.label.starts_with(&display) {
                        cloned.label = format!("{}: {}", display, cloned.label);
                    }
                }
                cloned
            })
            .collect();
    }

    let mut result = Vec::new();
    let mut nvme_groups: HashMap<String, Vec<&SensorReading>> = HashMap::new();

    for r in readings {
        if r.kind == DeviceKind::Nvme {
            nvme_groups.entry(r.device_name.clone()).or_default().push(r);
        } else {
            result.push(r.clone());
        }
    }

    let mut nvme_devices: Vec<String> = nvme_groups.keys().cloned().collect();
    nvme_devices.sort();

    for dev in nvme_devices {
        let list = &nvme_groups[&dev];
        // Prefer "Composite" sensor, otherwise the maximum temperature reading for the drive
        let chosen = list
            .iter()
            .find(|s| s.label.to_lowercase().contains("composite"))
            .unwrap_or_else(|| {
                list.iter()
                    .max_by(|a, b| a.temp.partial_cmp(&b.temp).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap()
            });

        let mut squashed = (*chosen).clone();
        // Clean single label: the drive's model (e.g. "Micron MTFDKCD5...") or,
        // when the model is unavailable, its kernel node (e.g. "nvme0").
        squashed.label = chosen.display_name();
        result.push(squashed);
    }

    // Keep sorted by kind and name
    result.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.device_name.cmp(&b.device_name))
            .then_with(|| a.label.cmp(&b.label))
    });

    result
}

/// Read CPU frequencies across active cores from sysfs or /proc/cpuinfo
pub fn read_cpu_freq() -> Option<CpuFreq> {
    let cpufreq_dir = Path::new("/sys/devices/system/cpu/cpufreq");
    let mut freqs_khz = Vec::new();

    if let Ok(entries) = fs::read_dir(cpufreq_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let cur_file = path.join("scaling_cur_freq");
            if let Ok(content) = fs::read_to_string(&cur_file) {
                if let Ok(khz) = content.trim().parse::<f64>() {
                    if khz > 0.0 {
                        freqs_khz.push(khz);
                    }
                }
            }
        }
    }

    // Fallback to /proc/cpuinfo if sysfs cpufreq is absent
    if freqs_khz.is_empty() {
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("cpu MHz") {
                    if let Some(val) = line.split(':').nth(1) {
                        if let Ok(mhz) = val.trim().parse::<f64>() {
                            freqs_khz.push(mhz * 1000.0);
                        }
                    }
                }
            }
        }
    }

    if freqs_khz.is_empty() {
        return None;
    }

    let max_khz = freqs_khz.iter().copied().fold(0.0, f64::max);
    let avg_khz = freqs_khz.iter().copied().sum::<f64>() / freqs_khz.len() as f64;

    Some(CpuFreq {
        max_ghz: max_khz / 1_000_000.0,
        avg_ghz: avg_khz / 1_000_000.0,
    })
}

/// Read GPU clock frequency from hwmon freq*_input or sysfs
pub fn read_gpu_freq() -> Option<f64> {
    let hwmon_dir = Path::new("/sys/class/hwmon");
    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = fs::read_to_string(path.join("name")).unwrap_or_default();
            let lower_name = name.trim().to_lowercase();
            if lower_name.contains("amdgpu")
                || lower_name.contains("nouveau")
                || lower_name.contains("nvidia")
                || lower_name.contains("i915")
            {
                // 1. Check freq1_input, freq2_input, etc. (reported in Hz)
                for i in 1..=4 {
                    let freq_file = path.join(format!("freq{}_input", i));
                    if let Ok(content) = fs::read_to_string(&freq_file) {
                        if let Ok(hz) = content.trim().parse::<f64>() {
                            if hz > 1_000_000.0 {
                                return Some(hz / 1_000_000_000.0); // Convert Hz to GHz
                            }
                        }
                    }
                }

                // 2. Check device/pp_dpm_sclk (AMD DPM clocks)
                let sclk_file = path.join("device").join("pp_dpm_sclk");
                if let Ok(content) = fs::read_to_string(&sclk_file) {
                    for line in content.lines() {
                        if line.contains('*') {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            for part in parts {
                                if part.to_lowercase().ends_with("mhz") {
                                    let num_str = &part[..part.len() - 3];
                                    if let Ok(mhz) = num_str.parse::<f64>() {
                                        return Some(mhz / 1000.0); // Convert MHz to GHz
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Check Intel DRM sysfs
    let drm_dir = Path::new("/sys/class/drm");
    if let Ok(entries) = fs::read_dir(drm_dir) {
        for entry in entries.filter_map(Result::ok) {
            let intel_freq = entry.path().join("gt_cur_freq_mhz");
            if let Ok(content) = fs::read_to_string(&intel_freq) {
                if let Ok(mhz) = content.trim().parse::<f64>() {
                    return Some(mhz / 1000.0);
                }
            }
        }
    }

    None
}

pub fn device_type(sensor: &SensorReading) -> &'static str {
    match sensor.kind {
        DeviceKind::Cpu => "CPU",
        DeviceKind::Gpu => "GPU",
        DeviceKind::Nvme => "NVME",
        DeviceKind::Memory => "RAM",
        DeviceKind::Motherboard => "MOBO",
        DeviceKind::Battery => "BAT",
        DeviceKind::Other => "OTHER",
    }
}

/// Read raw sector counts for each NVMe namespace device from /proc/diskstats.
/// Returns a map of base device name (e.g. "nvme0") → (read_sectors, write_sectors).
/// Linux kernel sector size is always 512 bytes.
pub fn read_nvme_diskstats() -> HashMap<String, (u64, u64)> {
    let mut map = HashMap::new();
    let content = match fs::read_to_string("/proc/diskstats") {
        Ok(c) => c,
        Err(_) => return map,
    };
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }
        let dev_name = parts[2];
        // Match nvme<N>n<M> (namespace devices, not partitions)
        // e.g. "nvme0n1", "nvme1n2" — must not end with a digit after 'p'
        if !dev_name.starts_with("nvme") {
            continue;
        }
        // Only capture the whole-disk namespace (nvme0n1, nvme1n1, etc.), not partitions
        // Partitions look like: nvme0n1p1, nvme1n1p2 — they contain 'p' after 'n'
        let after_nvme = &dev_name[4..]; // e.g. "0n1" or "0n1p1"
        if after_nvme.contains('p') {
            continue; // skip partitions
        }
        // Map "nvme0n1" → "nvme0" by stripping the namespace suffix "n<M>"
        let base_dev = if let Some(n_pos) = after_nvme.find('n') {
            format!("nvme{}", &after_nvme[..n_pos])
        } else {
            continue;
        };

        // diskstats columns (0-indexed): 0=major, 1=minor, 2=name, 3=reads_completed,
        // 4=reads_merged, 5=sectors_read, 6=read_ms, 7=writes_completed,
        // 8=writes_merged, 9=sectors_written, 10=write_ms, ...
        let read_sectors: u64 = parts[5].parse().unwrap_or(0);
        let write_sectors: u64 = parts[9].parse().unwrap_or(0);
        map.insert(base_dev, (read_sectors, write_sectors));
    }
    map
}

/// Format bytes/sec into a human-readable string: "12.3 MB/s", "456 KB/s", "789 B/s"
pub fn format_bytes_per_sec(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.1} GB/s", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0} KB/s", bps / 1_000.0)
    } else if bps > 0.0 {
        format!("{:.0} B/s", bps)
    } else {
        "0 B/s".to_string()
    }
}
