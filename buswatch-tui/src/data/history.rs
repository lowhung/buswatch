//! Historical data tracking for sparklines and rate calculations.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use super::monitor::MonitorData;

/// Maximum number of historical snapshots to keep.
const MAX_HISTORY_SIZE: usize = 60;

/// Tracks historical data for trending and sparklines.
///
/// Records snapshots over time to enable rate calculations and
/// visual trend indicators in the UI.
#[derive(Debug, Clone)]
pub struct History {
    /// Historical read counts per module (module_name -> readings).
    pub module_reads: HashMap<String, VecDeque<u64>>,
    /// Historical write counts per module.
    pub module_writes: HashMap<String, VecDeque<u64>>,
    /// Timestamps of snapshots for rate calculations.
    pub timestamps: VecDeque<Instant>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    /// Create a new empty history.
    pub fn new() -> Self {
        Self {
            module_reads: HashMap::new(),
            module_writes: HashMap::new(),
            timestamps: VecDeque::new(),
        }
    }

    /// Record a new data snapshot
    pub fn record(&mut self, data: &MonitorData) {
        // Record historical values for sparklines
        for module in &data.modules {
            let reads = self.module_reads.entry(module.name.clone()).or_default();
            reads.push_back(module.total_read);
            if reads.len() > MAX_HISTORY_SIZE {
                reads.pop_front();
            }

            let writes = self.module_writes.entry(module.name.clone()).or_default();
            writes.push_back(module.total_written);
            if writes.len() > MAX_HISTORY_SIZE {
                writes.pop_front();
            }
        }

        self.timestamps.push_back(data.last_updated);
        if self.timestamps.len() > MAX_HISTORY_SIZE {
            self.timestamps.pop_front();
        }
    }

    /// Get sparkline data for reads (normalized to 0-7 for 8 bar levels).
    ///
    /// Returns an empty Vec if there's not enough history.
    pub fn get_reads_sparkline(&self, module_name: &str) -> Vec<u8> {
        self.normalize_sparkline(self.module_reads.get(module_name))
    }

    /// Normalize values to 0-7 range for sparkline display.
    fn normalize_sparkline(&self, data: Option<&VecDeque<u64>>) -> Vec<u8> {
        let Some(values) = data else {
            return Vec::new();
        };

        if values.len() < 2 {
            return Vec::new();
        }

        // Compute deltas between consecutive values
        let deltas: Vec<i64> = values
            .iter()
            .zip(values.iter().skip(1))
            .map(|(a, b)| *b as i64 - *a as i64)
            .collect();

        if deltas.is_empty() {
            return Vec::new();
        }

        let max = deltas.iter().copied().max().unwrap_or(1).max(1);
        let min = deltas.iter().copied().min().unwrap_or(0).min(0);
        let range = (max - min).max(1) as f64;

        deltas
            .iter()
            .map(|&v| {
                let normalized = ((v - min) as f64 / range * 7.0) as u8;
                normalized.min(7)
            })
            .collect()
    }

    /// Get the rate of change (messages per second) for reads.
    ///
    /// Returns None if there's not enough history to calculate a rate.
    pub fn get_read_rate(&self, module_name: &str) -> Option<f64> {
        let reads = self.module_reads.get(module_name)?;
        if reads.len() < 2 || self.timestamps.len() < 2 {
            return None;
        }

        let current = *reads.back()?;
        let previous = *reads.get(reads.len() - 2)?;
        let delta = current as i64 - previous as i64;

        let current_time = self.timestamps.back()?;
        let previous_time = self.timestamps.get(self.timestamps.len() - 2)?;
        let elapsed = current_time.duration_since(*previous_time).as_secs_f64();

        if elapsed > 0.0 {
            Some(delta as f64 / elapsed)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::monitor::{HealthStatus, ModuleData, Thresholds};
    use buswatch_types::Snapshot;

    fn make_monitor_data(modules: Vec<(&str, u64, u64)>) -> MonitorData {
        let module_data: Vec<ModuleData> = modules
            .into_iter()
            .map(|(name, reads, writes)| ModuleData {
                name: name.to_string(),
                reads: vec![],
                writes: vec![],
                total_read: reads,
                total_written: writes,
                health: HealthStatus::Healthy,
            })
            .collect();

        MonitorData {
            modules: module_data,
            last_updated: Instant::now(),
        }
    }

    #[test]
    fn new_history_is_empty() {
        let h = History::new();
        assert!(h.module_reads.is_empty());
        assert!(h.module_writes.is_empty());
        assert!(h.timestamps.is_empty());
    }

    #[test]
    fn record_stores_module_data() {
        let mut h = History::new();
        let data = make_monitor_data(vec![("service", 100, 50)]);

        h.record(&data);

        assert!(h.module_reads.contains_key("service"));
        assert!(h.module_writes.contains_key("service"));
        assert_eq!(h.module_reads.get("service").unwrap().len(), 1);
    }

    #[test]
    fn record_accumulates_history() {
        let mut h = History::new();

        for i in 0..5 {
            let data = make_monitor_data(vec![("service", i * 10, i * 5)]);
            h.record(&data);
        }

        assert_eq!(h.module_reads.get("service").unwrap().len(), 5);
        assert_eq!(h.timestamps.len(), 5);
    }

    #[test]
    fn history_caps_at_max_size() {
        let mut h = History::new();

        // Record more than MAX_HISTORY_SIZE (60) entries
        for i in 0..70 {
            let data = make_monitor_data(vec![("service", i, 0)]);
            h.record(&data);
        }

        // Should be capped at 60
        assert_eq!(h.module_reads.get("service").unwrap().len(), 60);
        assert_eq!(h.timestamps.len(), 60);
    }

    #[test]
    fn sparkline_empty_for_unknown_module() {
        let h = History::new();
        let sparkline = h.get_reads_sparkline("unknown");
        assert!(sparkline.is_empty());
    }

    #[test]
    fn sparkline_empty_with_single_reading() {
        let mut h = History::new();
        let data = make_monitor_data(vec![("service", 100, 0)]);
        h.record(&data);

        let sparkline = h.get_reads_sparkline("service");
        assert!(sparkline.is_empty()); // Need at least 2 readings
    }

    #[test]
    fn sparkline_returns_normalized_deltas() {
        let mut h = History::new();

        // Record increasing values
        for i in 0..5 {
            let data = make_monitor_data(vec![("service", i * 100, 0)]);
            h.record(&data);
        }

        let sparkline = h.get_reads_sparkline("service");
        assert_eq!(sparkline.len(), 4); // 5 readings = 4 deltas

        // All deltas are equal (100), so all should normalize to same value
        assert!(sparkline.iter().all(|&v| v == sparkline[0]));
    }

    #[test]
    fn read_rate_none_for_unknown_module() {
        let h = History::new();
        assert!(h.get_read_rate("unknown").is_none());
    }

    #[test]
    fn read_rate_none_with_single_reading() {
        let mut h = History::new();
        let data = make_monitor_data(vec![("service", 100, 0)]);
        h.record(&data);

        assert!(h.get_read_rate("service").is_none());
    }

    #[test]
    fn read_rate_calculated_from_last_two_readings() {
        let mut h = History::new();

        let data1 = make_monitor_data(vec![("service", 100, 0)]);
        h.record(&data1);

        // Small delay to ensure non-zero elapsed time
        std::thread::sleep(std::time::Duration::from_millis(10));

        let data2 = make_monitor_data(vec![("service", 200, 0)]);
        h.record(&data2);

        let rate = h.get_read_rate("service");
        assert!(rate.is_some());
        // Rate should be positive (100 messages over ~10ms)
        assert!(rate.unwrap() > 0.0);
    }

    #[test]
    fn default_is_same_as_new() {
        let h1 = History::new();
        let h2 = History::default();

        assert_eq!(h1.module_reads.len(), h2.module_reads.len());
        assert_eq!(h1.timestamps.len(), h2.timestamps.len());
    }
}
