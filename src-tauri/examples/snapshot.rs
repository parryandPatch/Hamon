//! Prints one fully-shaped `Snapshot` as JSON.
//!
//! `Snapshot::default()` already serialises every field — nothing is skipped —
//! so this only has to make the lists non-empty for the element shapes to be
//! comparable. Used by `npm run check:wire` to confirm the frontend's
//! hand-written `types.ts`, and the synthetic snapshots in `mock.ts`, still
//! describe the shape the backend actually sends.

use hamon_lib::model::{
    BatterySample, CpuSample, DiskIoDevice, DiskIoSample, DiskSample, Filesystem, GpuDevice,
    GpuSample, MemorySample, NetInterface, NetworkSample, ProcessInfo, ProcessSample, Sensor,
    SensorKind, SensorSample, Snapshot, StorageKind, SystemInfo,
};

fn main() {
    let snapshot = Snapshot {
        seq: 0,
        timestamp_ms: 0,
        system: SystemInfo {
            hostname: "example".into(),
            distro: "Example 1.0 (x86_64)".into(),
            os_name: "Example".into(),
            os_version: "1.0".into(),
            kernel: "example-kernel".into(),
            arch: "x86_64".into(),
            model: "Example Machine".into(),
            cpu_brand: "Example CPU".into(),
            cpu_vendor: "Example".into(),
            cores_physical: 8,
            cores_logical: 16,
            gpu_names: vec!["Example GPU".into()],
            boot_time: 1,
            uptime_seconds: 2,
            privileged_hint: Some("example".into()),
        },
        cpu: CpuSample {
            usage: 1.0,
            per_core: vec![1.0, 2.0],
            user: 1.0,
            system: 1.0,
            idle: 90.0,
            iowait: 1.0,
            steal: 0.0,
            frequency_mhz: Some(3000.0),
            frequency_max_mhz: Some(4000.0),
            temperature: Some(50.0),
            load1: 1.0,
            load5: 1.0,
            load15: 1.0,
            thread_count: 100,
            process_count: 50,
            per_core_frequency_mhz: vec![Some(3000.0), None],
        },
        memory: MemorySample {
            total_bytes: 1,
            used_bytes: 1,
            available_bytes: 1,
            percent: 50.0,
            cached_bytes: 1,
            buffers_bytes: 1,
            swap_total_bytes: 1,
            swap_used_bytes: 1,
            swap_percent: 1.0,
        },
        gpu: GpuSample {
            devices: vec![GpuDevice {
                engines: vec![("3d".into(), 10.0)],
                ..Default::default()
            }],
        },
        network: NetworkSample {
            total_rx_bytes: 1,
            total_tx_bytes: 1,
            rx_bytes_per_sec: 1.0,
            tx_bytes_per_sec: 1.0,
            rx_packets_per_sec: 1.0,
            tx_packets_per_sec: 1.0,
            interfaces: vec![NetInterface {
                mac: Some("00:00:00:00:00:00".into()),
                ip: Some("127.0.0.1".into()),
                ..Default::default()
            }],
        },
        disk: DiskSample {
            filesystems: vec![Filesystem {
                fs_type: Some("ext4".into()),
                kind: StorageKind::Internal,
                ..Default::default()
            }],
        },
        disk_io: DiskIoSample {
            read_bytes_per_sec: 1.0,
            write_bytes_per_sec: 1.0,
            read_bytes_total: 1,
            write_bytes_total: 1,
            busy_percent: Some(1.0),
            devices: vec![DiskIoDevice {
                model: Some("example disk".into()),
                ..Default::default()
            }],
        },
        sensors: SensorSample {
            sensors: vec![Sensor {
                label: "example".into(),
                kind: SensorKind::Cpu,
                temp_c: 50.0,
                high_c: Some(90.0),
                critical_c: Some(100.0),
                watts: Some(10.0),
                millivolts: Some(1200.0),
                rpm: Some(1000.0),
                needs_privileges: false,
            }],
        },
        battery: BatterySample {
            present: true,
            percentage: Some(50.0),
            is_charging: false,
            ac_connected: true,
            power_watts: Some(10.0),
            voltage_mv: Some(12000.0),
            design_capacity_mah: Some(5000),
            full_charge_capacity_mah: Some(4800),
            cycle_count: Some(100),
            health_percent: Some(96.0),
            temperature_c: Some(30.0),
            time_to_empty_minutes: Some(120),
            time_to_full_minutes: None,
        },
        processes: ProcessSample {
            top_cpu: vec![ProcessInfo {
                user: Some("example".into()),
                ..Default::default()
            }],
            top_memory: vec![ProcessInfo {
                user: Some("example".into()),
                ..Default::default()
            }],
        },
    };

    match serde_json::to_string_pretty(&snapshot) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialise the sample snapshot: {error}");
            std::process::exit(1);
        }
    }
}
