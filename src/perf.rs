// src/perf.rs
// Battery-focused performance monitoring for CardBrick
// Tracks actual power consumption, GPU frequency, and thermal state

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Performance metrics collected over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSample {
    pub timestamp_ns: u64,
    pub scene: String,

    // Battery metrics (primary concern)
    pub power_mw: Option<f64>,   // Instantaneous power draw
    pub energy_mwh: Option<f64>, // Cumulative energy since start

    // GPU metrics
    pub gpu_freq_khz: Option<u32>,         // Current GPU frequency
    pub gpu_transition_count: Option<u32>, // GPU frequency changes

    // CPU metrics (process-specific)
    pub cpu_percent: Option<f64>,   // % of single core used by CardBrick
    pub memory_rss_mb: Option<f64>, // Resident memory in MB

    // Thermal metrics
    pub temp_celsius: Option<f64>, // SoC temperature

    // Rendering metrics
    pub fps: Option<f64>,           // Frames per second
    pub frame_time_ms: Option<f64>, // Time to render last frame
}

/// Ring buffer for performance samples
pub struct PerfMonitor {
    samples: VecDeque<PerfSample>,
    start_time: Instant,
    last_battery_reading: Option<(f64, u64)>, // (mWh, timestamp_ns)
    last_cpu_stat: Option<CpuStat>,
    current_scene: String,
    max_samples: usize,

    // Hardware paths (may not exist on all systems)
    battery_current_path: Option<String>,
    battery_voltage_path: Option<String>,
    gpu_freq_path: Option<String>,
    gpu_trans_path: Option<String>,
    thermal_path: Option<String>,
}

#[derive(Debug, Clone)]
struct CpuStat {
    utime: u64,
    stime: u64,
    timestamp_ns: u64,
}

impl PerfMonitor {
    /// Create a new performance monitor
    /// Only available when compiled with "perf" feature
    #[cfg(feature = "perf")]
    pub fn new() -> Self {
        let mut monitor = Self {
            samples: VecDeque::with_capacity(1000), // ~17 minutes at 1Hz
            start_time: Instant::now(),
            last_battery_reading: None,
            last_cpu_stat: None,
            current_scene: "unknown".to_string(),
            max_samples: 1000,
            battery_current_path: None,
            battery_voltage_path: None,
            gpu_freq_path: None,
            gpu_trans_path: None,
            thermal_path: None,
        };

        monitor.discover_hardware_paths();
        monitor
    }

    /// Stub implementation when perf feature is disabled
    #[cfg(not(feature = "perf"))]
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            start_time: Instant::now(),
            last_battery_reading: None,
            last_cpu_stat: None,
            current_scene: "unknown".to_string(),
            max_samples: 0,
            battery_current_path: None,
            battery_voltage_path: None,
            gpu_freq_path: None,
            gpu_trans_path: None,
            thermal_path: None,
        }
    }

    /// Discover available hardware monitoring paths
    #[cfg(feature = "perf")]
    fn discover_hardware_paths(&mut self) {
        // Look for AXP PMIC battery monitoring
        let battery_supplies = [
            "/sys/class/power_supply/axp2202-battery",
            "/sys/class/power_supply/axp20x-battery",
            "/sys/class/power_supply/battery",
        ];

        for supply in &battery_supplies {
            let current_path = format!("{}/current_now", supply);
            let voltage_path = format!("{}/voltage_now", supply);

            if fs::metadata(&current_path).is_ok() && fs::metadata(&voltage_path).is_ok() {
                self.battery_current_path = Some(current_path);
                self.battery_voltage_path = Some(voltage_path);
                println!("[perf] Found battery monitoring at {}", supply);
                break;
            }
        }

        // Look for GPU devfreq monitoring (H700 SoC / Panfrost)
        let gpu_paths = ["/sys/class/devfreq/1800000.gpu", "/sys/class/devfreq/gpu"];

        for gpu_path in &gpu_paths {
            let freq_path = format!("{}/cur_freq", gpu_path);
            let trans_path = format!("{}/trans_stat", gpu_path);

            if fs::metadata(&freq_path).is_ok() {
                self.gpu_freq_path = Some(freq_path);
                if fs::metadata(&trans_path).is_ok() {
                    self.gpu_trans_path = Some(trans_path);
                }
                println!("[perf] Found GPU monitoring at {}", gpu_path);
                break;
            }
        }

        // Look for thermal monitoring
        let thermal_paths = [
            "/sys/class/thermal/thermal_zone0/temp",
            "/sys/class/thermal/thermal_zone1/temp",
        ];

        for thermal_path in &thermal_paths {
            if fs::metadata(thermal_path).is_ok() {
                self.thermal_path = Some(thermal_path.to_string());
                println!("[perf] Found thermal monitoring at {}", thermal_path);
                break;
            }
        }

        println!("[perf] Performance monitoring initialized");
    }

    #[cfg(not(feature = "perf"))]
    fn discover_hardware_paths(&mut self) {
        // No-op when perf feature disabled
    }

    /// Set the current scene being measured
    pub fn set_scene(&mut self, scene: &str) {
        self.current_scene = scene.to_string();

        #[cfg(feature = "perf")]
        println!("[perf] Scene changed to: {}", scene);
    }

    /// Take a performance sample
    #[cfg(feature = "perf")]
    pub fn sample(&mut self) {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let sample = PerfSample {
            timestamp_ns,
            scene: self.current_scene.clone(),
            power_mw: self.read_power(),
            energy_mwh: self.calculate_energy(timestamp_ns),
            gpu_freq_khz: self.read_gpu_freq(),
            gpu_transition_count: self.read_gpu_transitions(),
            cpu_percent: self.read_cpu_percent(timestamp_ns),
            memory_rss_mb: self.read_memory_mb(),
            temp_celsius: self.read_temperature(),
            fps: Some(macroquad::time::get_fps() as f64),
            frame_time_ms: Some(macroquad::time::get_frame_time() as f64 * 1000.0),
        };

        // Add to ring buffer
        self.samples.push_back(sample);
        if self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    /// No-op when perf feature disabled
    #[cfg(not(feature = "perf"))]
    pub fn sample(&mut self) {
        // No-op
    }

    /// Read instantaneous power draw in milliwatts
    #[cfg(feature = "perf")]
    fn read_power(&self) -> Option<f64> {
        let current_path = self.battery_current_path.as_ref()?;
        let voltage_path = self.battery_voltage_path.as_ref()?;

        let current_ua = fs::read_to_string(current_path)
            .ok()?
            .trim()
            .parse::<i64>()
            .ok()? as f64; // microamps

        let voltage_uv = fs::read_to_string(voltage_path)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()? as f64; // microvolts

        // Convert to milliwatts: (µA * µV) / 1_000_000 = mW
        Some((current_ua * voltage_uv) / 1_000_000.0)
    }

    /// Calculate cumulative energy consumption
    #[cfg(feature = "perf")]
    fn calculate_energy(&mut self, timestamp_ns: u64) -> Option<f64> {
        let power_mw = self.read_power()?;

        if let Some((last_energy, last_time)) = self.last_battery_reading {
            let dt_s = (timestamp_ns - last_time) as f64 / 1_000_000_000.0;
            let energy_mwh = last_energy + (power_mw * dt_s / 3600.0); // mWh
            self.last_battery_reading = Some((energy_mwh, timestamp_ns));
            Some(energy_mwh)
        } else {
            // First reading
            self.last_battery_reading = Some((0.0, timestamp_ns));
            Some(0.0)
        }
    }

    /// Read current GPU frequency in kHz
    #[cfg(feature = "perf")]
    fn read_gpu_freq(&self) -> Option<u32> {
        let path = self.gpu_freq_path.as_ref()?;
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    /// Read GPU frequency transition count (changes indicate load)
    #[cfg(feature = "perf")]
    fn read_gpu_transitions(&self) -> Option<u32> {
        let path = self.gpu_trans_path.as_ref()?;
        let content = fs::read_to_string(path).ok()?;

        // Parse trans_stat format: matrix of transitions
        // Sum all transition counts as a rough activity metric
        let total: u32 = content
            .lines()
            .skip(1) // Skip header
            .flat_map(|line| line.split_whitespace().skip(1)) // Skip frequency column
            .filter_map(|s| s.parse::<u32>().ok())
            .sum();

        Some(total)
    }

    /// Read process-specific CPU usage percentage
    #[cfg(feature = "perf")]
    fn read_cpu_percent(&mut self, timestamp_ns: u64) -> Option<f64> {
        let stat_content = fs::read_to_string("/proc/self/stat").ok()?;
        let fields: Vec<&str> = stat_content.split_whitespace().collect();

        // Fields 13 and 14 are utime and stime (user and system CPU time)
        let utime = fields.get(13)?.parse::<u64>().ok()?;
        let stime = fields.get(14)?.parse::<u64>().ok()?;

        let current_stat = CpuStat {
            utime,
            stime,
            timestamp_ns,
        };

        if let Some(last_stat) = &self.last_cpu_stat {
            let dt_ns = timestamp_ns - last_stat.timestamp_ns;
            let dt_s = dt_ns as f64 / 1_000_000_000.0;

            let d_utime = current_stat.utime - last_stat.utime;
            let d_stime = current_stat.stime - last_stat.stime;
            let d_total = d_utime + d_stime;

            // Convert CPU ticks to percentage (assuming 100 ticks per second)
            let cpu_percent = (d_total as f64 / 100.0) / dt_s * 100.0;

            self.last_cpu_stat = Some(current_stat);
            Some(cpu_percent.min(100.0)) // Cap at 100%
        } else {
            self.last_cpu_stat = Some(current_stat);
            Some(0.0)
        }
    }

    /// Read memory usage in MB
    #[cfg(feature = "perf")]
    fn read_memory_mb(&self) -> Option<f64> {
        let status_content = fs::read_to_string("/proc/self/status").ok()?;

        for line in status_content.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let kb = parts.get(1)?.parse::<f64>().ok()?;
                return Some(kb / 1024.0); // Convert KB to MB
            }
        }

        None
    }

    /// Read SoC temperature in Celsius
    #[cfg(feature = "perf")]
    fn read_temperature(&self) -> Option<f64> {
        let path = self.thermal_path.as_ref()?;
        let millicelsius = fs::read_to_string(path).ok()?.trim().parse::<i32>().ok()?;

        Some(millicelsius as f64 / 1000.0)
    }

    /// Get all collected samples
    pub fn get_samples(&self) -> &VecDeque<PerfSample> {
        &self.samples
    }

    /// Save samples to JSON file
    #[cfg(feature = "perf")]
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(&self.samples)?;
        fs::write(path, json)?;
        println!("[perf] Saved {} samples to {}", self.samples.len(), path);
        Ok(())
    }

    #[cfg(not(feature = "perf"))]
    pub fn save_to_file(&self, _path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // No-op when perf feature disabled
        Ok(())
    }

    /// Clear all samples
    pub fn clear(&mut self) {
        self.samples.clear();
        self.last_battery_reading = None;
        self.last_cpu_stat = None;

        #[cfg(feature = "perf")]
        println!("[perf] Cleared all samples");
    }

    /// Get performance summary statistics
    #[cfg(feature = "perf")]
    pub fn get_summary(&self) -> PerfSummary {
        if self.samples.is_empty() {
            return PerfSummary::default();
        }

        let mut fps_values = Vec::new();
        let mut power_values = Vec::new();
        let mut cpu_values = Vec::new();
        let mut memory_values = Vec::new();
        let mut temp_values = Vec::new();

        for sample in &self.samples {
            if let Some(fps) = sample.fps {
                fps_values.push(fps);
            }
            if let Some(power) = sample.power_mw {
                power_values.push(power);
            }
            if let Some(cpu) = sample.cpu_percent {
                cpu_values.push(cpu);
            }
            if let Some(mem) = sample.memory_rss_mb {
                memory_values.push(mem);
            }
            if let Some(temp) = sample.temp_celsius {
                temp_values.push(temp);
            }
        }

        PerfSummary {
            sample_count: self.samples.len(),
            duration_s: self.start_time.elapsed().as_secs_f64(),
            avg_fps: mean(&fps_values),
            avg_power_mw: mean(&power_values),
            avg_cpu_percent: mean(&cpu_values),
            avg_memory_mb: mean(&memory_values),
            max_temp_celsius: temp_values.iter().fold(0.0, |a, &b| a.max(b)),
            total_energy_mwh: self.samples.back().and_then(|s| s.energy_mwh),
        }
    }

    #[cfg(not(feature = "perf"))]
    pub fn get_summary(&self) -> PerfSummary {
        PerfSummary::default()
    }
}

impl Default for PerfMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance summary statistics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PerfSummary {
    pub sample_count: usize,
    pub duration_s: f64,
    pub avg_fps: Option<f64>,
    pub avg_power_mw: Option<f64>,
    pub avg_cpu_percent: Option<f64>,
    pub avg_memory_mb: Option<f64>,
    pub max_temp_celsius: f64,
    pub total_energy_mwh: Option<f64>,
}

/// Calculate mean of a vector, returning None if empty
fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// Global performance monitor instance
static PERF_MONITOR: OnceLock<Mutex<PerfMonitor>> = OnceLock::new();

/// Initialize global performance monitor
pub fn init_perf_monitor() {
    let _ = PERF_MONITOR.set(Mutex::new(PerfMonitor::new()));
}

/// Get reference to global performance monitor
pub fn perf_monitor() -> Option<MutexGuard<'static, PerfMonitor>> {
    PERF_MONITOR.get().and_then(|monitor| monitor.lock().ok())
}

/// Convenience function to set scene
pub fn set_perf_scene(scene: &str) {
    if let Some(mut monitor) = perf_monitor() {
        monitor.set_scene(scene);
    }
}

/// Convenience function to take sample
pub fn perf_sample() {
    if let Some(mut monitor) = perf_monitor() {
        monitor.sample();
    }
}

/// Convenience function to save results
pub fn save_perf_results(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(monitor) = perf_monitor() {
        monitor.save_to_file(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_monitor_creation() {
        let monitor = PerfMonitor::new();
        assert_eq!(monitor.current_scene, "unknown");
        assert!(monitor.samples.is_empty());
    }

    #[test]
    fn test_scene_setting() {
        let mut monitor = PerfMonitor::new();
        monitor.set_scene("testing");
        assert_eq!(monitor.current_scene, "testing");
    }

    #[test]
    fn test_mean_calculation() {
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[1.0]), Some(1.0));
        assert_eq!(mean(&[1.0, 2.0, 3.0]), Some(2.0));
    }

    #[cfg(feature = "perf")]
    #[test]
    fn test_path_discovery() {
        let mut monitor = PerfMonitor::new();
        monitor.discover_hardware_paths();
        // Just verify it doesn't panic - actual paths depend on hardware
    }
}
