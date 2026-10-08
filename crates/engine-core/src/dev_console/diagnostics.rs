use std::fmt::Write as _;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub const INSPECTOR_INTERVAL: Duration = Duration::from_millis(500);

pub use deflorta_common::diagnostics::{GpuStats, LoadedAsset};

pub struct ProcessSampler {
    system: System,
    pid: Option<Pid>,
    sampled: Option<Instant>,
    pub values: Vec<(String, String)>,
}

impl Default for ProcessSampler {
    fn default() -> Self {
        Self {
            system: System::new(),
            pid: sysinfo::get_current_pid().ok(),
            sampled: None,
            values: Vec::new(),
        }
    }
}

impl ProcessSampler {
    pub fn sample(&mut self, now: Instant) {
        if self
            .sampled
            .is_some_and(|last| now.duration_since(last) < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL)
        {
            return;
        }
        let previous = self.sampled.replace(now);
        self.values.clear();
        let Some(pid) = self.pid.filter(|_| sysinfo::IS_SUPPORTED_SYSTEM) else {
            self.values.push((
                "Process telemetry".into(),
                "Unavailable on this platform".into(),
            ));
            return;
        };
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .without_tasks(),
        );
        let Some(process) = self.system.process(pid) else {
            self.values
                .push(("Process telemetry".into(), "Process is unavailable".into()));
            return;
        };
        let io = process.disk_usage();
        self.values.extend([
            ("Process ID".into(), pid.to_string()),
            (
                "Process CPU (100% = one core)".into(),
                if previous.is_none() {
                    "Warming up; request stats again after 500 ms".into()
                } else {
                    format!("{:.1}%", process.cpu_usage())
                },
            ),
            ("Resident memory (RSS)".into(), bytes(process.memory())),
            ("Virtual memory".into(), bytes(process.virtual_memory())),
            (
                "Process disk read / written".into(),
                format!(
                    "{} / {}",
                    bytes(io.total_read_bytes),
                    bytes(io.total_written_bytes)
                ),
            ),
            ("Process uptime".into(), format!("{} s", process.run_time())),
        ]);
    }
}

#[must_use]
pub fn bytes(value: u64) -> String {
    use num_traits::ToPrimitive as _;
    format!(
        "{:.2} MiB",
        value.to_f64().unwrap_or(f64::MAX) / 1_048_576.0
    )
}

pub fn asset_report(assets: &[LoadedAsset]) -> String {
    let mut report = format!(
        "{} loaded asset records (source = asset ID; streamed media has no full-file memory estimate):\n",
        assets.len()
    );
    for asset in assets {
        writeln!(
            report,
            "{} | {} | {} | {} | {}",
            asset.kind,
            asset.source,
            asset.state,
            asset.detail,
            asset.bytes.map_or_else(|| "memory: n/a".into(), bytes)
        )
        .unwrap();
    }
    report
}

#[must_use]
pub fn stats_report(values: &[(String, String)]) -> String {
    values
        .iter()
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[must_use]
pub fn tree_report(tree: &accesskit::TreeUpdate) -> String {
    let mut report = format!(
        "Accessibility tree: {} nodes, focus #{}\n",
        tree.nodes.len(),
        tree.focus.0
    );
    for (id, node) in &tree.nodes {
        writeln!(report, "#{} {node:?}", id.0).unwrap();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_sampling_warms_up_and_throttles_repeated_requests() {
        let mut sampler = ProcessSampler::default();
        assert_eq!(sampler.values.len(), 0);
        let now = Instant::now();
        sampler.sample(now);
        let first = sampler.values.clone();
        assert_ne!(first.len(), 0);
        sampler.sample(now + Duration::from_millis(1));
        assert_eq!(sampler.values, first);
        assert_eq!(sampler.sampled, Some(now));
        if sysinfo::IS_SUPPORTED_SYSTEM {
            assert!(first.iter().any(|(label, value)| {
                label == "Process CPU (100% = one core)" && value.contains("Warming up")
            }));
            assert!(
                first
                    .iter()
                    .any(|(label, _)| label == "Resident memory (RSS)")
            );
        }
    }
}
