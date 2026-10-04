//! The sampler: one background thread that ticks and broadcasts.
//!
//! Why a thread instead of sampling inside a Tauri command:
//!
//! * Command handlers run on the webview's async runtime, and a synchronous
//!   `ioreg`/`sysfs` sweep would block the UI thread.
//! * The frontend wants a push stream, not polling. One event per tick keeps
//!   the IPC surface to a single `listen`.
//!
//! Cadences are deliberately mixed: fast metrics (CPU, network, disk I/O)
//! tick at the requested interval, while expensive ones (process list,
//! filesystem enumeration, static system info) run on a slower schedule.
//! The slower values are carried forward unchanged between refreshes.

use crate::collect::{
    battery::BatteryCollector, cpu::CpuCollector, disk::DiskCollector, gpu::GpuCollector,
    network::NetCollector, process::ProcessCollector, sensors::SensorCollector, system,
};
use crate::model::{DiskSample, ProcessSample, Snapshot, SystemInfo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Sample interval bounds. Below the minimum the collectors spend more time
/// shelling out than sampling, and above the maximum the UI feels dead.
pub const MIN_INTERVAL_MS: u64 = 200;
pub const MAX_INTERVAL_MS: u64 = 10_000;

/// How often the expensive collectors refresh.
const PROCESS_INTERVAL_MS: u64 = 2_500;
const FILESYSTEM_INTERVAL_MS: u64 = 10_000;
const SYSTEM_INTERVAL_MS: u64 = 30_000;

/// The callback the sampling thread invokes once per tick.
type Emit = Arc<dyn Fn(Snapshot) + Send + Sync + 'static>;

/// Shared sampling handle. Cloning is cheap.
///
/// Lifecycle is `spawn` (start), [`Sampler::stop`] (pause) and
/// [`Sampler::resume`] (un-pause). `stop` is sticky until `resume` is called,
/// which is why `running` and "is the thread actually alive" are tracked
/// separately: after a stop the thread winds down asynchronously, and a resume
/// that raced it must still end up with exactly one thread running.
#[derive(Clone)]
pub struct Sampler {
    interval: Arc<Mutex<Duration>>,
    running: Arc<AtomicBool>,
    /// `(flag, condvar)` pair used to cut the sleep short on demand.
    wake: Arc<(Mutex<bool>, Condvar)>,
    /// Retained so [`Sampler::resume`] can restart the thread without the
    /// caller having to supply the callback again.
    emit: Arc<Mutex<Option<Emit>>>,
    /// Serialises `spawn`/`resume` against the thread's own exit path.
    life: Arc<Mutex<()>>,
    /// Only ever read or written while `life` is held.
    thread_alive: Arc<AtomicBool>,
}

impl Sampler {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval: Arc::new(Mutex::new(clamp_interval(interval_ms))),
            // `new` alone does not start anything; `spawn` or `resume` does.
            running: Arc::new(AtomicBool::new(false)),
            wake: Arc::new((Mutex::new(false), Condvar::new())),
            emit: Arc::new(Mutex::new(None)),
            life: Arc::new(Mutex::new(())),
            thread_alive: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval
            .lock()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1000)
    }

    /// Validates and applies a new interval. Returns the value actually used.
    pub fn set_interval_ms(&self, ms: u64) -> u64 {
        let clamped = clamp_interval(ms);
        if let Ok(mut d) = self.interval.lock() {
            *d = clamped;
        }
        clamped.as_millis() as u64
    }

    /// Asks the sampling thread to exit after its current tick.
    ///
    /// Idempotent, and safe to call before anything was ever spawned.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.notify();
    }

    /// Restarts sampling, using the callback given to [`Sampler::spawn`].
    ///
    /// A no-op when already running. Called from the UI's play/pause toggle.
    pub fn resume(&self) {
        self.start();
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Signals the sampling thread to produce a snapshot immediately instead
    /// of waiting out the remainder of its interval.
    ///
    /// Used when the window regains focus, so the numbers on screen match the
    /// moment the user is actually looking at them.
    pub fn notify(&self) {
        let (lock, cvar) = &*self.wake;
        if let Ok(mut pending) = lock.lock() {
            *pending = true;
        }
        cvar.notify_all();
    }

    /// Sleeps until the interval elapses or [`Sampler::notify`] is called,
    /// whichever comes first. Returns `false` if the sampler was stopped.
    fn sleep_until(&self, remaining: Duration) -> bool {
        if remaining.is_zero() {
            return self.is_running();
        }
        let (lock, cvar) = &*self.wake;
        let Ok(pending) = lock.lock() else {
            return self.is_running();
        };
        let (mut guard, _timeout) = cvar
            .wait_timeout_while(pending, remaining, |p| !*p)
            .unwrap_or_else(|e| e.into_inner());
        *guard = false;
        self.is_running()
    }

    /// Installs the emit callback and starts the thread if it is not running.
    pub fn spawn<F>(&self, emit: F)
    where
        F: Fn(Snapshot) + Send + Sync + 'static,
    {
        {
            let mut slot = self.emit.lock().unwrap_or_else(|e| e.into_inner());
            *slot = Some(Arc::new(emit));
        }
        self.start();
    }

    /// Brings the sampler to "running", spawning a thread if needed.
    fn start(&self) {
        let emit = {
            let slot = self.emit.lock().unwrap_or_else(|e| e.into_inner());
            match slot.clone() {
                Some(e) => e,
                // No callback installed yet: there is nothing to emit to.
                // `spawn` is the only public way in, so this is unreachable in
                // practice, but failing soft beats panicking a UI thread.
                None => {
                    log::warn!("start() called before spawn(); sampler left stopped");
                    return;
                }
            }
        };

        let _life = self.life.lock().unwrap_or_else(|e| e.into_inner());

        // Set before the thread can observe it, so the thread's own exit path
        // cannot clear the flag for a thread that is already running again.
        self.running.store(true, Ordering::SeqCst);
        if self.thread_alive.load(Ordering::SeqCst) {
            // A thread exists; it will pick up `running` on its next check.
            self.notify();
            return;
        }
        self.thread_alive.store(true, Ordering::SeqCst);

        let this = self.clone();

        let spawned = std::thread::Builder::new()
            .name("hamon-sampler".into())
            .spawn(move || {
                let mut state = SamplerState::new();
                // Force the first pass through the slow collectors too.
                let mut last_process = Instant::now() - Duration::from_millis(PROCESS_INTERVAL_MS);
                let mut last_filesystem =
                    Instant::now() - Duration::from_millis(FILESYSTEM_INTERVAL_MS);
                let mut last_system = Instant::now() - Duration::from_millis(SYSTEM_INTERVAL_MS);

                let mut seq: u64 = 0;
                while this.is_running() {
                    let tick_start = Instant::now();

                    if last_process.elapsed().as_millis() as u64 >= PROCESS_INTERVAL_MS {
                        state.processes = Some(state.process.sample());
                        last_process = tick_start;
                    }
                    if last_filesystem.elapsed().as_millis() as u64 >= FILESYSTEM_INTERVAL_MS {
                        state.disk_cache = Some(state.cpu.filesystems());
                        last_filesystem = tick_start;
                    }
                    if last_system.elapsed().as_millis() as u64 >= SYSTEM_INTERVAL_MS {
                        state.system = system::collect(state.cores, state.gpu.names());
                        last_system = tick_start;
                    }

                    let snapshot = state.tick(seq);
                    seq += 1;
                    emit(snapshot);

                    // Sleep out the remainder of the interval so a slow tick
                    // does not push subsequent ticks further out than asked.
                    // `notify` cuts the wait short without restarting the
                    // cadence, which is why this is a timed wait rather than a
                    // plain `thread::sleep`.
                    let target = this
                        .interval
                        .lock()
                        .map(|d| *d)
                        .unwrap_or(Duration::from_secs(1));
                    if !this.sleep_until(target.saturating_sub(tick_start.elapsed())) {
                        break;
                    }
                }

                // Take the lifecycle lock before clearing the flag so a
                // concurrent `resume` either sees this thread still alive (and
                // leaves it running) or waits here and starts a fresh one.
                let _life = this.life.lock().unwrap_or_else(|e| e.into_inner());
                this.thread_alive.store(false, Ordering::SeqCst);
                log::info!("sampler thread stopped");
            });

        if let Err(e) = spawned {
            self.thread_alive.store(false, Ordering::SeqCst);
            self.running.store(false, Ordering::SeqCst);
            log::error!("could not spawn sampler thread: {e}");
        }
    }
}

fn clamp_interval(ms: u64) -> Duration {
    Duration::from_millis(ms.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS))
}

/// Holds every collector, plus the slowly-refreshed values between updates.
struct SamplerState {
    cpu: CpuCollector,
    net: NetCollector,
    disk: DiskCollector,
    gpu: GpuCollector,
    sensors: SensorCollector,
    battery: BatteryCollector,
    process: ProcessCollector,

    /// Fixed for the process lifetime; the UI sizes its core grid from this.
    cores: usize,
    /// Cached: refreshed every [`SYSTEM_INTERVAL_MS`].
    system: SystemInfo,
    /// Cached: refreshed every [`FILESYSTEM_INTERVAL_MS`].
    disk_cache: Option<DiskSample>,
    /// Cached: refreshed every [`PROCESS_INTERVAL_MS`].
    processes: Option<ProcessSample>,
}

impl SamplerState {
    fn new() -> Self {
        let mut cpu = CpuCollector::new();
        let cores = cpu.logical_cores();

        // GPU discovery shells out to `ioreg` / `dlopen`s NVML, so it happens
        // once here and its result is reused for every `SystemInfo` refresh.
        let gpu = GpuCollector::new();
        let gpu_names = gpu.names();
        let system = system::collect(cores, gpu_names);

        // A priming pass so the very first emitted snapshot is complete
        // rather than a screen full of zeroes.
        let disk_cache = Some(cpu.filesystems());

        let mut process = ProcessCollector::new();
        let processes = Some(process.sample());

        Self {
            cpu,
            net: NetCollector::new(),
            disk: DiskCollector::new(),
            gpu,
            sensors: SensorCollector::new(),
            battery: BatteryCollector::new(),
            process,
            cores,
            system,
            disk_cache,
            processes,
        }
    }

    fn tick(&mut self, seq: u64) -> Snapshot {
        let cpu = self.cpu.sample();
        let memory = self.cpu.memory();
        let network = self.net.sample();
        let disk_io = self.disk.io();
        let gpu = self.gpu.sample();
        let sensors = self.sensors.sample();

        // Promote the hottest CPU sensor to the headline temperature.
        let cpu_temp = self.sensors.cpu_temperature(&sensors);

        Snapshot {
            seq,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            system: self.system.clone(),
            cpu: crate::model::CpuSample {
                temperature: cpu_temp,
                process_count: self.process.total(),
                ..cpu
            },
            memory,
            gpu,
            network,
            disk: self.disk_cache.clone().unwrap_or_default(),
            disk_io,
            sensors,
            battery: self.battery.sample(),
            processes: self.processes.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn interval_is_clamped_to_bounds() {
        let s = Sampler::new(50);
        assert_eq!(s.interval_ms(), MIN_INTERVAL_MS);
        let s = Sampler::new(60_000);
        assert_eq!(s.interval_ms(), MAX_INTERVAL_MS);
        let s = Sampler::new(1000);
        assert_eq!(s.set_interval_ms(10), MIN_INTERVAL_MS);
        assert_eq!(s.set_interval_ms(750), 750);
    }

    #[test]
    fn emits_snapshots_and_stops() {
        let (tx, rx) = mpsc::channel();
        let sampler = Sampler::new(MIN_INTERVAL_MS);
        sampler.spawn(move |snap| {
            let _ = tx.send(snap);
        });

        let first = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no snapshot");
        assert_eq!(first.seq, 0);
        assert!(!first.cpu.per_core.is_empty());
        assert!(first.memory.total_bytes > 0);
        assert!(!first.system.distro.is_empty());

        // Sequence numbers must advance so the UI can drop stale frames.
        let second = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no second snapshot");
        assert!(second.seq > first.seq);

        sampler.stop();
        sampler.stop(); // idempotent
        assert!(!sampler.is_running());
    }

    #[test]
    fn stop_then_resume_restarts_the_stream() {
        let (tx, rx) = mpsc::channel();
        let sampler = Sampler::new(MIN_INTERVAL_MS);
        sampler.spawn(move |snap| {
            let _ = tx.send(snap);
        });

        rx.recv_timeout(Duration::from_secs(5))
            .expect("no first snapshot");
        sampler.stop();

        // Drain whatever was already in flight, then confirm the stream goes
        // quiet. One extra tick may already have been queued, so allow for it.
        while rx.recv_timeout(Duration::from_secs(5)).is_ok() {}
        assert!(
            rx.recv_timeout(Duration::from_millis(400)).is_err(),
            "sampler kept ticking after stop()"
        );

        sampler.resume();
        assert!(sampler.is_running());
        rx.recv_timeout(Duration::from_secs(5))
            .expect("resume did not restart the stream");

        sampler.stop();
    }

    #[test]
    fn resume_without_spawn_is_harmless() {
        let sampler = Sampler::new(MIN_INTERVAL_MS);
        sampler.resume();
        assert!(!sampler.is_running(), "there is no callback to emit to");
    }

    #[test]
    fn spawn_is_idempotent() {
        let (tx, rx) = mpsc::channel::<crate::model::Snapshot>();
        let first_sender = tx.clone();
        let sampler = Sampler::new(MIN_INTERVAL_MS);
        sampler.spawn(move |s| {
            let _ = first_sender.send(s);
        });
        // A second `spawn` replaces the callback but must not start a second
        // thread; the seq numbers below would interleave if it did.
        sampler.spawn(move |s| {
            let _ = tx.send(s);
        });

        let first = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no snapshot");
        let second = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("no second snapshot");
        assert_eq!(first.seq, 0);
        assert_eq!(second.seq, 1);
        sampler.stop();
    }

    #[test]
    fn first_snapshot_is_fully_populated() {
        let mut state = SamplerState::new();
        let snap = state.tick(0);
        assert_eq!(snap.seq, 0);
        assert!(snap.timestamp_ms > 1_600_000_000_000, "clock looks wrong");
        assert_eq!(snap.cpu.per_core.len(), snap.cpu.thread_count);
        assert!(snap.cpu.usage.is_finite());
        assert!(snap.memory.percent.is_finite());
        assert!(
            !snap.disk.filesystems.is_empty(),
            "expected at least one mount"
        );
        assert!(
            !snap.processes.top_cpu.is_empty(),
            "expected at least one process"
        );
    }

    #[test]
    fn repeated_ticks_keep_shapes_stable() {
        let mut state = SamplerState::new();
        let first = state.tick(0);
        let second = state.tick(1);
        assert_eq!(first.cpu.per_core.len(), second.cpu.per_core.len());
        assert_eq!(first.disk.filesystems.len(), second.disk.filesystems.len());
        assert!(second.timestamp_ms >= first.timestamp_ms);
    }
}
