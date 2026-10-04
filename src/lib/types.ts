/**
 * TypeScript mirrors of the Rust wire types in `src-tauri/src/model.rs`,
 * `layout.rs` and `commands.rs`.
 *
 * The backend serialises with `#[derive(Serialize)]` and no `rename_all`, so
 * every field name here is snake_case exactly as declared in Rust. Fields that
 * are `Option<T>` in Rust are `T | null` here and arrive as `null` rather than
 * being absent — the distinction matters, because a null is how the UI learns
 * "this platform cannot report this", which is not the same as zero.
 */

export interface SystemInfo {
  hostname: string;
  distro: string;
  os_name: string;
  os_version: string;
  kernel: string;
  arch: string;
  model: string;
  cpu_brand: string;
  cpu_vendor: string;
  cores_physical: number;
  cores_logical: number;
  gpu_names: string[];
  boot_time: number;
  uptime_seconds: number;
  privileged_hint: string | null;
}

export interface CpuSample {
  usage: number;
  per_core: number[];
  user: number;
  system: number;
  idle: number;
  iowait: number;
  steal: number;
  frequency_mhz: number | null;
  frequency_max_mhz: number | null;
  temperature: number | null;
  load1: number;
  load5: number;
  load15: number;
  thread_count: number;
  process_count: number;
  per_core_frequency_mhz: (number | null)[];
}

export interface MemorySample {
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  percent: number;
  cached_bytes: number;
  buffers_bytes: number;
  swap_total_bytes: number;
  swap_used_bytes: number;
  swap_percent: number;
}

export interface GpuDevice {
  index: number;
  name: string;
  vendor: string;
  kind: string;
  usage_percent: number | null;
  /** `[engineName, utilisationPercent]` pairs. */
  engines: [string, number][];
  memory_used_bytes: number | null;
  memory_total_bytes: number | null;
  temperature: number | null;
  power_watts: number | null;
  power_limit_watts: number | null;
  fan_percent: number | null;
  clock_mhz: number | null;
  memory_clock_mhz: number | null;
  driver_version: string | null;
}

export interface GpuSample {
  devices: GpuDevice[];
}

export interface NetInterface {
  name: string;
  is_up: boolean;
  is_loopback: boolean;
  mac: string | null;
  ip: string | null;
  rx_bytes: number;
  tx_bytes: number;
  rx_bytes_per_sec: number;
  tx_bytes_per_sec: number;
  rx_errors: number;
  tx_errors: number;
}

export interface NetworkSample {
  total_rx_bytes: number;
  total_tx_bytes: number;
  rx_bytes_per_sec: number;
  tx_bytes_per_sec: number;
  rx_packets_per_sec: number;
  tx_packets_per_sec: number;
  interfaces: NetInterface[];
}

export type StorageKind = 'internal' | 'removable' | 'network' | 'unknown';

export interface Filesystem {
  name: string;
  mount_point: string;
  fs_type: string | null;
  total_bytes: number;
  used_bytes: number;
  free_bytes: number;
  percent: number;
  kind: StorageKind;
}

export interface DiskSample {
  filesystems: Filesystem[];
}

export interface DiskIoDevice {
  name: string;
  model: string | null;
  read_bytes_per_sec: number;
  write_bytes_per_sec: number;
}

export interface DiskIoSample {
  read_bytes_per_sec: number;
  write_bytes_per_sec: number;
  read_bytes_total: number;
  write_bytes_total: number;
  busy_percent: number | null;
  devices: DiskIoDevice[];
}

export type SensorKind =
  | 'cpu'
  | 'gpu'
  | 'memory'
  | 'nvme'
  | 'storage'
  | 'battery'
  | 'fan'
  | 'other';

export interface Sensor {
  label: string;
  kind: SensorKind;
  temp_c: number;
  high_c: number | null;
  critical_c: number | null;
  watts: number | null;
  millivolts: number | null;
  rpm: number | null;
  needs_privileges: boolean;
}

export interface SensorSample {
  sensors: Sensor[];
}

export interface BatterySample {
  present: boolean;
  percentage: number | null;
  is_charging: boolean;
  ac_connected: boolean;
  power_watts: number | null;
  voltage_mv: number | null;
  design_capacity_mah: number | null;
  full_charge_capacity_mah: number | null;
  cycle_count: number | null;
  health_percent: number | null;
  temperature_c: number | null;
  time_to_empty_minutes: number | null;
  time_to_full_minutes: number | null;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  user: string | null;
  cpu_percent: number;
  memory_bytes: number;
}

export interface ProcessSample {
  top_cpu: ProcessInfo[];
  top_memory: ProcessInfo[];
}

export interface Snapshot {
  seq: number;
  timestamp_ms: number;
  system: SystemInfo;
  cpu: CpuSample;
  memory: MemorySample;
  gpu: GpuSample;
  network: NetworkSample;
  disk: DiskSample;
  disk_io: DiskIoSample;
  sensors: SensorSample;
  battery: BatterySample;
  processes: ProcessSample;
}

export interface LayoutWidget {
  kind: string;
  config: unknown;
}

export interface Layout {
  version: number;
  widgets: LayoutWidget[];
  columns: number;
  sample_interval_ms: number;
}

export interface RuntimeStatus {
  running: boolean;
  interval_ms: number;
}

export interface IntervalBounds {
  min_ms: number;
  max_ms: number;
}

export interface Bootstrap {
  layout: Layout;
  status: RuntimeStatus;
  bounds: IntervalBounds;
}