/**
 * A fake backend for developing the UI in a plain browser.
 *
 * `npm run dev` outside a Tauri window has no IPC, so every command would
 * reject and the dashboard would be a screenshot of an error state. This module
 * implements the same command surface against synthetic data, which means the
 * whole frontend — including edit mode, drag-to-reorder, the configuration panel
 * and the charts — can be exercised, and visually regression-checked, without
 * rebuilding Rust.
 *
 * It is only ever selected when the Tauri internals are absent, so it cannot
 * shadow the real backend. `tauri.ts` picks between the two.
 *
 * The numbers are deliberately *lively* rather than constant: a flat line makes
 * a sparkline look broken, and the whole point of a chart is that it moves.
 */

import type {
  Bootstrap,
  DiskIoSample,
  GpuDevice,
  Layout,
  NetInterface,
  ProcessInfo,
  RuntimeStatus,
  Snapshot,
} from './types';

/** Deterministic noise, so a reload looks like the same machine. */
function wobble(seed: number, t: number, amplitude: number, period = 7): number {
  const phase = (seed % 97) * 0.37;
  const slow = Math.sin(t / period + phase);
  const faster = Math.sin(t / (period / 3.7) + phase * 2);
  const noise = Math.sin(t * 2.3 + seed * 1.7) * 0.25;
  return (slow * 0.6 + faster * 0.28 + noise * 0.12) * amplitude;
}

/** Clamps into `0..=max`. */
function bounded(value: number, max: number): number {
  return Math.min(max, Math.max(0, value));
}

const BOOT_TIME = Math.floor(Date.now() / 1000) - 6 * 86400 - 5 * 3600 - 42 * 60;

const NAMES = [
  'Safari', 'kernel_task', 'Code Helper', 'Hamon', 'WindowServer', 'zsh', 'node',
  'cargo', 'rustc', 'Google Chrome', 'Finder', 'mds_stores', 'cloudflared', 'sshd',
  'postgres', 'Docker', 'Slack', 'Terminal', 'ffmpeg',
];

const USERS = ['apple', 'root', '_windowserver', '_networkd'];

function processes(t: number, count: number): ProcessInfo[] {
  const rows: ProcessInfo[] = [];
  for (let i = 0; i < count; i++) {
    const cpu = bounded(wobble(i, t, 34) + (i === 1 ? 22 : 4), 100);
    rows.push({
      pid: 100 + i * 7,
      name: NAMES[i % NAMES.length],
      user: i % 5 === 0 ? null : USERS[i % USERS.length],
      cpu_percent: Math.round(cpu * 10) / 10,
      memory_bytes: Math.round(Math.max(1, 90e6 * (1 + wobble(i + 40, t / 3, 1.4, 21))) + i * 30e6),
    });
  }
  return rows;
}

function interfaces(t: number): NetInterface[] {
  const make = (
    name: string,
    isLoopback: boolean,
    mac: string | null,
    ip: string | null,
    up: boolean,
    scale: number,
    phase: number,
    base: { rx: number; tx: number },
  ): NetInterface => {
    const rx = isLoopback ? wobble(3, t * 4, 4e6) : bounded(wobble(phase, t, scale, 5), 26e6);
    const tx = isLoopback ? rx : bounded(wobble(phase + 11, t, scale * 0.42, 8), 9e6);
    return {
      name,
      is_up: up,
      is_loopback: isLoopback,
      mac,
      ip,
      rx_bytes: base.rx + Math.abs(rx) * 60,
      tx_bytes: base.tx + Math.abs(tx) * 60,
      rx_bytes_per_sec: up ? Math.abs(rx) : 0,
      tx_bytes_per_sec: up ? Math.abs(tx) : 0,
      rx_errors: 0,
      tx_errors: 0,
    };
  };
  return [
    make('lo0', true, null, '127.0.0.1', true, 0, 0, { rx: 4e9, tx: 4e9 }),
    make('en0', false, '3c:22:fb:1a:9d:41', '192.168.1.24', true, 3.2e6, 1, { rx: 5.02e10, tx: 1.4e9 }),
    make('en1', false, '3c:22:fb:1a:9d:42', null, true, 0, 2, { rx: 0, tx: 0 }),
    make('utun3', false, null, '100.96.0.2', true, 9.5e6, 5, { rx: 3.0e9, tx: 8.4e9 }),
    make('utun5', false, null, null, true, 0, 7, { rx: 0, tx: 0 }),
  ];
}

function gpu(t: number): GpuDevice[] {
  const load = bounded(wobble(21, t, 46, 11), 92);
  return [
    {
      index: 0,
      name: 'Apple M1 Pro',
      vendor: 'Apple',
      kind: 'integrated',
      usage_percent: Math.round(load * 10) / 10,
      engines: [
        ['GPU activity', Math.round(load * 10) / 10],
        ['3D', Math.round(bounded(wobble(31, t, 30, 6), 100))],
        ['Compute', Math.round(bounded(wobble(41, t, 18, 9), 100))],
      ],
      memory_used_bytes: Math.round(Math.abs(wobble(51, t / 4, 1.6e9, 13)) + 5.4e9),
      memory_total_bytes: null,
      temperature: null,
      power_watts: null,
      power_limit_watts: null,
      fan_percent: null,
      clock_mhz: Math.round(400 + wobble(61, t / 6, 120, 17)),
      memory_clock_mhz: null,
      driver_version: null,
    },
  ];
}

function diskIo(t: number): DiskIoSample {
  const read = bounded(wobble(71, t, 2.4e6, 6), 180e6);
  const write = bounded(wobble(73, t, 3.1e6, 9), 240e6);
  return {
    read_bytes_per_sec: read,
    write_bytes_per_sec: write,
    read_bytes_total: 1.4e12 + Math.abs(wobble(75, t, 1e9)),
    write_bytes_total: 2.1e12 + Math.abs(wobble(77, t, 1e9)),
    busy_percent: bounded(((read + write) / 2.4e9) * 100, 100),
    devices: [
      { name: 'disk0', model: 'APPLE SSD AP0256Z', read_bytes_per_sec: read, write_bytes_per_sec: write },
      { name: 'disk0s2', model: null, read_bytes_per_sec: read * 0.08, write_bytes_per_sec: write * 0.03 },
    ],
  };
}

/**
 * A synthetic snapshot with every field the backend sends.
 *
 * Exported so `npm run check:wire` can compare its shape against the JSON that
 * `cargo run --example snapshot` produces — that is what catches a field being
 * renamed on the Rust side, which typechecking cannot see because `types.ts` is
 * a hand-written mirror rather than generated from the real types.
 */
export function syntheticSnapshot(seq: number, t: number): Snapshot {
  const net = interfaces(t);
  const live = net.filter((i) => i.is_up && !i.is_loopback);
  const totalRx = live.reduce((sum, i) => sum + i.rx_bytes_per_sec, 0);
  const totalTx = live.reduce((sum, i) => sum + i.tx_bytes_per_sec, 0);
  const memoryTotal = 34_359_738_368;
  const memoryUsed = Math.abs(wobble(81, t / 5, 5e9, 31)) + 9.5e9;
  const cpuUsage = bounded(wobble(83, t, 38, 13), 100);

  return {
    seq,
    timestamp_ms: Date.now(),
    system: {
      hostname: 'hamon-dev',
      distro: 'macOS 26.0 (arm64)',
      os_name: 'macOS',
      os_version: '26.0',
      kernel: 'Darwin Kernel Version 26.0.0',
      arch: 'arm64',
      model: 'MacBook Pro (M1 Pro, 14-inch, 2021)',
      cpu_brand: 'Apple M1 Pro',
      cpu_vendor: 'Apple',
      cores_physical: 10,
      cores_logical: 10,
      gpu_names: ['Apple M1 Pro'],
      boot_time: BOOT_TIME,
      uptime_seconds: Math.floor(Date.now() / 1000) - BOOT_TIME,
      privileged_hint: null,
    },
    cpu: {
      usage: Math.round(cpuUsage * 10) / 10,
      per_core: Array.from({ length: 10 }, (_, i) => Math.round(bounded(wobble(90 + i, t, 42, 4 + i), 100))),
      user: bounded(cpuUsage * 0.7, 100),
      system: bounded(cpuUsage * 0.22, 100),
      idle: 100 - bounded(cpuUsage, 100),
      iowait: bounded(wobble(95, t, 6, 17), 100),
      steal: 0,
      frequency_mhz: 2100 + wobble(97, t / 8, 500, 19),
      frequency_max_mhz: 3200,
      temperature: null,
      load1: 1.42 + wobble(99, t / 30, 0.8, 41),
      load5: 1.71,
      load15: 1.63,
      thread_count: 487 + Math.round(wobble(101, t / 12, 40, 37)),
      process_count: 512,
      per_core_frequency_mhz: Array.from({ length: 10 }, (_, i) =>
        Math.round(2100 + wobble(110 + i, t / 8, 420, 15 + i)),
      ),
    },
    memory: {
      total_bytes: memoryTotal,
      used_bytes: memoryUsed,
      available_bytes: memoryTotal - memoryUsed,
      percent: Math.round((100 * memoryUsed) / memoryTotal * 10) / 10,
      cached_bytes: 6.1e9,
      buffers_bytes: 213e6,
      swap_total_bytes: 0,
      swap_used_bytes: 0,
      swap_percent: 0,
    },
    gpu: { devices: gpu(t) },
    network: {
      total_rx_bytes: live.reduce((sum, i) => sum + i.rx_bytes, 0),
      total_tx_bytes: live.reduce((sum, i) => sum + i.tx_bytes, 0),
      rx_bytes_per_sec: totalRx,
      tx_bytes_per_sec: totalTx,
      rx_packets_per_sec: totalRx / 1400,
      tx_packets_per_sec: totalTx / 1600,
      interfaces: net,
    },
    disk: {
      filesystems: [
        {
          name: '/dev/disk3s1s1',
          mount_point: '/',
          fs_type: 'apfs',
          total_bytes: 494_384_795_648,
          used_bytes: 61_284_451_328,
          free_bytes: 78_452_398_080,
          percent: 44.5,
          kind: 'internal',
        },
        {
          name: '/dev/disk3s6',
          mount_point: '/System/Volumes/Data',
          fs_type: 'apfs',
          total_bytes: 494_384_795_648,
          used_bytes: 322_411_744_256,
          free_bytes: 78_452_398_080,
          percent: 80.4,
          kind: 'internal',
        },
      ],
    },
    disk_io: diskIo(t),
    sensors: {
      sensors: [
        { label: 'PECI CPU', kind: 'cpu', temp_c: 47.8 + wobble(120, t / 3, 4, 23), high_c: 95, critical_c: 100, watts: 5.4 + wobble(121, t, 2, 5), millivolts: null, rpm: null, needs_privileges: true },
        { label: 'TG3P Proximity', kind: 'other', temp_c: 38.2 + wobble(122, t / 4, 1.5, 29), high_c: null, critical_c: null, watts: null, millivolts: null, rpm: null, needs_privileges: true },
        { label: 'Battery', kind: 'battery', temp_c: 31.4 + wobble(123, t / 9, 0.6, 37), high_c: 45, critical_c: 55, watts: 4.1 + wobble(124, t, 1.6, 7), millivolts: 12600, rpm: null, needs_privileges: true },
        { label: 'Right side', kind: 'other', temp_c: 33.1 + wobble(125, t / 5, 1.1, 19), high_c: null, critical_c: null, watts: null, millivolts: null, rpm: null, needs_privileges: true },
        { label: 'Left side', kind: 'other', temp_c: 32.6 + wobble(126, t / 5, 1.1, 21), high_c: null, critical_c: null, watts: null, millivolts: null, rpm: null, needs_privileges: true },
      ],
    },
    battery: {
      present: true,
      percentage: Math.round(72 - wobble(130, t / 40, 8, 53)),
      is_charging: false,
      ac_connected: true,
      power_watts: 4.2 + wobble(131, t, 1.8, 6),
      voltage_mv: 12640,
      design_capacity_mah: 5085,
      full_charge_capacity_mah: 4712,
      cycle_count: 312,
      health_percent: 92.7,
      temperature_c: 31.4,
      time_to_empty_minutes: null,
      time_to_full_minutes: null,
    },
    processes: {
      top_cpu: processes(t, 10).sort((a, b) => b.cpu_percent - a.cpu_percent),
      top_memory: processes(t + 5, 10).sort((a, b) => b.memory_bytes - a.memory_bytes),
    },
  };
}

/**
 * The in-browser stand-in for the Rust side of the app.
 *
 * Layout edits are persisted to `localStorage` so a reload behaves like a real
 * restart, including the "the config panel remembers what I set" case.
 */
export class MockBackend {
  #listeners = new Set<(snapshot: Snapshot) => void>();
  #timer: ReturnType<typeof setInterval> | null = null;
  #seq = 0;
  #started = Date.now();
  #running = true;
  #intervalMs = 1000;
  #layout: Layout = DEFAULT_LAYOUT;

  constructor() {
    const stored = localStorage.getItem('hamon.dev.layout');
    if (stored) {
      try {
        this.#layout = { ...DEFAULT_LAYOUT, ...(JSON.parse(stored) as Layout) };
      } catch {
        /* a corrupt dev fixture is not worth reporting */
      }
    }
  }

  #persist(): void {
    localStorage.setItem('hamon.dev.layout', JSON.stringify(this.#layout));
  }

  #tick(): void {
    if (!this.#running) return;
    const payload = syntheticSnapshot(this.#seq++, (Date.now() - this.#started) / 1000);
    for (const listener of this.#listeners) listener(payload);
  }

  onSnapshot = async (listener: (snapshot: Snapshot) => void): Promise<() => void> => {
    this.#listeners.add(listener);
    if (this.#timer === null) {
      this.#timer = setInterval(() => this.#tick(), this.#intervalMs);
    }
    this.#tick();
    return () => {
      this.#listeners.delete(listener);
      if (this.#listeners.size === 0 && this.#timer !== null) {
        clearInterval(this.#timer);
        this.#timer = null;
      }
    };
  };

  bootstrap = async (): Promise<Bootstrap> => ({
    layout: this.#layout,
    status: { running: this.#running, interval_ms: this.#intervalMs },
    bounds: { min_ms: 250, max_ms: 5000 },
  });

  getLayout = async (): Promise<Layout> => this.#layout;

  setLayout = async (layout: Layout): Promise<Layout> => {
    this.#layout = { ...layout, version: 1 };
    this.#persist();
    return this.#layout;
  };

  addWidget = async (kind: string): Promise<Layout> => {
    this.#layout = {
      ...this.#layout,
      widgets: [...this.#layout.widgets, { kind, config: {} }],
    };
    this.#persist();
    return this.#layout;
  };

  removeWidget = async (index: number): Promise<Layout> => {
    this.#layout = {
      ...this.#layout,
      widgets: this.#layout.widgets.filter((_, i) => i !== index),
    };
    this.#persist();
    return this.#layout;
  };

  moveWidget = async (from: number, to: number): Promise<Layout> => {
    const widgets = [...this.#layout.widgets];
    const [moved] = widgets.splice(from, 1);
    if (moved === undefined) throw new Error(`no widget at index ${from}`);
    widgets.splice(Math.max(0, Math.min(widgets.length, to)), 0, moved);
    this.#layout = { ...this.#layout, widgets };
    this.#persist();
    return this.#layout;
  };

  configureWidget = async (index: number, config: unknown): Promise<Layout> => {
    const widgets = this.#layout.widgets.map((w, i) => (i === index ? { ...w, config } : w));
    this.#layout = { ...this.#layout, widgets };
    this.#persist();
    return this.#layout;
  };

  setSampleInterval = async (intervalMs: number): Promise<Layout> => {
    this.#intervalMs = Math.min(5000, Math.max(250, Math.round(intervalMs)));
    if (this.#timer !== null) {
      clearInterval(this.#timer);
      this.#timer = setInterval(() => this.#tick(), this.#intervalMs);
    }
    this.#layout = { ...this.#layout, sample_interval_ms: this.#intervalMs };
    this.#persist();
    return this.#layout;
  };

  resetLayout = async (): Promise<Layout> => {
    this.#layout = DEFAULT_LAYOUT;
    this.#persist();
    return this.#layout;
  };

  getStatus = async (): Promise<RuntimeStatus> => ({
    running: this.#running,
    interval_ms: this.#intervalMs,
  });

  refresh = async (): Promise<void> => this.#tick();

  pauseSampling = async (): Promise<void> => {
    this.#running = false;
  };

  resumeSampling = async (): Promise<void> => {
    this.#running = true;
  };
}

/** Mirrors `layout.rs::default_widgets`, so the dev fixture looks like a real first run. */
const DEFAULT_LAYOUT: Layout = {
  version: 1,
  columns: 2,
  sample_interval_ms: 1000,
  widgets: [
    { kind: 'gauge', config: { metric: 'cpu-usage' } },
    { kind: 'cpu-cores', config: { show_frequency: true } },
    { kind: 'memory', config: {} },
    { kind: 'gpu', config: { device: 'auto' } },
    { kind: 'network', config: {} },
    { kind: 'disk', config: {} },
    { kind: 'temperatures', config: { show_fans: true } },
    { kind: 'system', config: { style: 'compact' } },
  ],
};
