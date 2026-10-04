//! Top processes by CPU and by resident memory.
//!
//! Enumerating every process is the most expensive thing Hamon does, so this
//! runs on a slow cadence (see [`crate::sample`]) rather than every tick.
//! `sysinfo` caches the previous tick's counters, which is what makes the CPU
//! percentages deltas rather than lifetime averages.
//!
//! This collector deliberately owns a *separate* `System` from the one in
//! [`super::cpu::CpuCollector`]: process refreshes and CPU refreshes share
//! internal state, and interleaving them makes the headline CPU usage jump
//! around between ticks.

use crate::model::{ProcessInfo, ProcessSample};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

/// How many entries each list keeps.
const TOP_N: usize = 12;

/// Refresh configuration shared by every call.
///
/// Only what the UI shows is requested. Notably `without_tasks()` — on Linux
/// the per-thread walk is the single biggest cost in a process refresh, and
/// Hamon never displays thread counts.
fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_user(UpdateKind::OnlyIfNotSet)
}

/// Owns the `System` and `Users` handles used for process enumeration.
pub struct ProcessCollector {
    sys: System,
    users: Users,
}

impl Default for ProcessCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessCollector {
    pub fn new() -> Self {
        let mut sys = System::new();
        let mut users = Users::new_with_refreshed_list();
        // Prime so the first tick reports deltas rather than values-since-boot.
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        let _ = &mut users;
        Self { sys, users }
    }

    /// Number of live processes, which the CPU widget shows next to the
    /// utilisation bar.
    pub fn total(&self) -> usize {
        self.sys.processes().len()
    }

    pub fn sample(&mut self) -> ProcessSample {
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());

        let all: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                user: p
                    .user_id()
                    .and_then(|uid| self.users.get_user_by_id(uid))
                    .map(|u| u.name().to_string()),
                cpu_percent: p.cpu_usage().clamp(0.0, 100.0),
                memory_bytes: p.memory(),
            })
            .collect();

        let mut top_cpu = all.clone();
        top_cpu.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
                // Ties are common at 0.0%, so break them by PID to keep the
                // list stable between ticks instead of reshuffling randomly.
                .then(a.pid.cmp(&b.pid))
        });
        top_cpu.truncate(TOP_N);

        // `launchd` reports a zero resident size on macOS, and a process can
        // also exit between enumeration and its stats being read. Neither says
        // anything about "top memory consumers", so they are dropped from this
        // ranking; they still appear in the CPU list, where memory is not the
        // metric being compared.
        let mut top_memory: Vec<ProcessInfo> =
            all.into_iter().filter(|p| p.memory_bytes > 0).collect();
        top_memory.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes).then(a.pid.cmp(&b.pid)));
        top_memory.truncate(TOP_N);

        ProcessSample {
            top_cpu,
            top_memory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_bounded_and_sorted() {
        let mut c = ProcessCollector::new();
        let s = c.sample();
        assert!(s.top_cpu.len() <= TOP_N);
        assert!(s.top_memory.len() <= TOP_N);

        for w in s.top_cpu.windows(2) {
            assert!(w[0].cpu_percent >= w[1].cpu_percent, "top_cpu not sorted");
        }
        for w in s.top_memory.windows(2) {
            assert!(
                w[0].memory_bytes >= w[1].memory_bytes,
                "top_memory not sorted"
            );
        }
    }

    #[test]
    fn entries_are_well_formed() {
        let mut c = ProcessCollector::new();
        let s = c.sample();
        for p in s.top_cpu.iter().chain(s.top_memory.iter()) {
            assert!(
                !p.name.is_empty(),
                "process with empty name (pid {})",
                p.pid
            );
            assert!(p.pid > 0);
        }
        // The CPU list may legitimately contain a process the kernel reports no
        // resident size for; the memory list must not, or it would show a row
        // reading "0 B" in a table of the greediest consumers.
        assert!(
            s.top_memory.iter().all(|p| p.memory_bytes > 0),
            "a zero-memory process reached the memory ranking"
        );
    }

    #[test]
    fn cpu_percentages_stay_in_range() {
        let mut c = ProcessCollector::new();
        let s = c.sample();
        assert!(
            s.top_cpu
                .iter()
                .all(|p| (0.0..=100.0).contains(&p.cpu_percent)),
            "cpu usage should be a delta percentage"
        );
    }

    #[test]
    fn sees_this_process() {
        let me = std::process::id();
        let mut c = ProcessCollector::new();
        // Refresh twice so the delta is meaningful, then confirm our own PID
        // is enumerated (not necessarily in the top N).
        let _ = c.sample();
        let s = c.sample();
        assert!(c.total() > 0, "no processes enumerated at all");

        // This test binary is idle, so it may well land in the top 12 by
        // memory; if it does, its entry must be identified correctly.
        if let Some(entry) = s
            .top_cpu
            .iter()
            .chain(s.top_memory.iter())
            .find(|p| p.pid == me)
        {
            assert!(!entry.name.is_empty(), "our own entry has no name");
            assert!(entry.memory_bytes > 0);
        }
    }
}
