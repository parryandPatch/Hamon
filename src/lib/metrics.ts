/**
 * Turns a `Snapshot` into the single number a gauge or chart widget shows.
 *
 * Both `gauge` and `history` widgets are configured with a metric name, so the
 * mapping from name to value lives here once. The backend has an equivalent
 * `Snapshot::gauge` for its own tests; this is the UI-side counterpart and the
 * two must agree on what "unavailable" looks like — `null`, never zero.
 */

import type { Snapshot } from './types';

export type MetricId =
  | 'cpu-usage'
  | 'cpu-frequency'
  | 'cpu-temperature'
  | 'memory-usage'
  | 'swap-usage'
  | 'gpu-usage'
  | 'gpu-temperature'
  | 'gpu-power'
  | 'gpu-memory'
  | 'gpu-fan'
  | 'net-rx'
  | 'net-tx'
  | 'disk-read'
  | 'disk-write'
  | 'disk-usage'
  | 'battery-level'
  | 'battery-power';

export interface MetricSpec {
  id: MetricId;
  label: string;
  /** How to render the raw number. */
  format: (value: number) => string;
  /**
   * Fixed upper bound for charts, or `null` to auto-scale. Percentages get a
   * fixed 0-100 axis so a CPU graph never looks dramatic when idle.
   */
  max: number | null;
  unit: string;
}

const fixed = (digits: number) => (v: number) => `${v.toFixed(digits)}%`;

export const METRICS: Record<MetricId, MetricSpec> = {
  'cpu-usage': { id: 'cpu-usage', label: 'CPU usage', format: fixed(0), max: 100, unit: '%' },
  'cpu-frequency': {
    id: 'cpu-frequency',
    label: 'CPU frequency',
    format: (v) => (v >= 1000 ? `${(v / 1000).toFixed(2)} GHz` : `${v.toFixed(0)} MHz`),
    max: null,
    unit: 'MHz',
  },
  'cpu-temperature': { id: 'cpu-temperature', label: 'CPU temperature', format: (v) => `${v.toFixed(1)} °C`, max: 100, unit: '°C' },
  'memory-usage': { id: 'memory-usage', label: 'Memory usage', format: fixed(0), max: 100, unit: '%' },
  'swap-usage': { id: 'swap-usage', label: 'Swap usage', format: fixed(0), max: 100, unit: '%' },
  'gpu-usage': { id: 'gpu-usage', label: 'GPU usage', format: fixed(0), max: 100, unit: '%' },
  'gpu-temperature': { id: 'gpu-temperature', label: 'GPU temperature', format: (v) => `${v.toFixed(1)} °C`, max: 100, unit: '°C' },
  'gpu-power': { id: 'gpu-power', label: 'GPU power', format: (v) => (v >= 1 ? `${v.toFixed(1)} W` : `${(v * 1000).toFixed(0)} mW`), max: null, unit: 'W' },
  'gpu-memory': {
    id: 'gpu-memory',
    label: 'GPU memory',
    format: (v) => `${(v / 1024 ** 3).toFixed(2)} GiB`,
    max: null,
    unit: 'B',
  },
  'gpu-fan': { id: 'gpu-fan', label: 'GPU fan', format: fixed(0), max: 100, unit: '%' },
  'net-rx': {
    id: 'net-rx',
    label: 'Download',
    format: (v) => `${v < 1024 ? `${v.toFixed(0)} B` : `${(v / 1024).toFixed(1)} KiB`}/s`,
    max: null,
    unit: 'B/s',
  },
  'net-tx': {
    id: 'net-tx',
    label: 'Upload',
    format: (v) => `${v < 1024 ? `${v.toFixed(0)} B` : `${(v / 1024).toFixed(1)} KiB`}/s`,
    max: null,
    unit: 'B/s',
  },
  'disk-read': {
    id: 'disk-read',
    label: 'Disk read',
    format: (v) => `${v < 1024 ? `${v.toFixed(0)} B` : `${(v / 1024).toFixed(1)} KiB`}/s`,
    max: null,
    unit: 'B/s',
  },
  'disk-write': {
    id: 'disk-write',
    label: 'Disk write',
    format: (v) => `${v < 1024 ? `${v.toFixed(0)} B` : `${(v / 1024).toFixed(1)} KiB`}/s`,
    max: null,
    unit: 'B/s',
  },
  'disk-usage': { id: 'disk-usage', label: 'Disk usage', format: fixed(0), max: 100, unit: '%' },
  'battery-level': { id: 'battery-level', label: 'Battery', format: fixed(0), max: 100, unit: '%' },
  'battery-power': {
    id: 'battery-power',
    label: 'Battery power',
    format: (v) => (v >= 1 ? `${v.toFixed(1)} W` : `${(v * 1000).toFixed(0)} mW`),
    max: null,
    unit: 'W',
  },
};

/** Metrics usable on a chart, and therefore worth keeping history for. */
export const CHARTABLE: MetricId[] = [
  'cpu-usage',
  'memory-usage',
  'gpu-usage',
  'gpu-temperature',
  'net-rx',
  'net-tx',
  'disk-read',
  'disk-write',
];

/**
 * The current value of one metric, or `null` when the platform cannot report
 * it. `null` and `0` mean different things throughout: the widgets render the
 * first as an em dash and the second as a number.
 *
 * Metrics that span several devices need a rule for which device to report, and
 * the rule depends on what the metric is *for*:
 *
 * - **Load, temperature, fan — the worst one.** A gauge reading the idle GPU's
 *   temperature while the busy one throttles is worse than no reading, because
 *   it looks calm. `max` also happens to be the interesting device for a single
 *   number.
 * - **Power, memory — the sum.** Two GPUs drawing 100 W each is 200 W, and one
 *   GPU's worth of memory is not what the machine is using. Summing is the only
 *   honest reduction for a quantity that adds up.
 * - **Disk usage — the fullest.** Same reasoning as GPU temperature: the volume
 *   about to run out is the one worth a widget.
 *
 * A sum that comes to zero is reported as `null`, since no device reporting a
 * power or memory figure at all is an absence of data, not a total of nothing.
 */
export function readMetric(snapshot: Snapshot | null, metric: string): number | null {
  if (!snapshot) return null;
  const s = snapshot;
  switch (metric) {
    case 'cpu-usage':
      return s.cpu.usage;
    case 'cpu-frequency':
      return s.cpu.frequency_mhz;
    case 'cpu-temperature':
      return s.cpu.temperature;
    case 'memory-usage':
      return s.memory.percent;
    case 'swap-usage':
      return s.memory.swap_total_bytes > 0 ? s.memory.swap_percent : null;
    case 'gpu-usage':
      return maxOf(s.gpu.devices.map((d) => d.usage_percent));
    case 'gpu-temperature':
      return maxOf(s.gpu.devices.map((d) => d.temperature));
    case 'gpu-power': {
      // Summed, not maxed: two GPUs drawing power is more than either alone.
      const total = s.gpu.devices.reduce((sum, d) => sum + (d.power_watts ?? 0), 0);
      return total > 0 ? total : null;
    }
    case 'gpu-memory': {
      const used = s.gpu.devices.reduce((sum, d) => sum + (d.memory_used_bytes ?? 0), 0);
      return used > 0 ? used : null;
    }
    case 'gpu-fan':
      return maxOf(s.gpu.devices.map((d) => d.fan_percent));
    case 'net-rx':
      return s.network.rx_bytes_per_sec;
    case 'net-tx':
      return s.network.tx_bytes_per_sec;
    case 'disk-read':
      return s.disk_io.read_bytes_per_sec;
    case 'disk-write':
      return s.disk_io.write_bytes_per_sec;
    case 'disk-usage':
      return s.disk.filesystems.length > 0 ? Math.max(...s.disk.filesystems.map((f) => f.percent)) : null;
    case 'battery-level':
      return s.battery.percentage;
    case 'battery-power':
      return s.battery.power_watts;
    default:
      return null;
  }
}

/** The busiest GPU device, which is what the GPU widget leads with. */
export function primaryGpu(snapshot: Snapshot | null) {
  if (!snapshot || snapshot.gpu.devices.length === 0) return null;
  return snapshot.gpu.devices.reduce((worst, d) => {
    const a = d.usage_percent ?? -1;
    const b = worst.usage_percent ?? -1;
    return a > b ? d : worst;
  });
}

function maxOf(values: (number | null)[]): number | null {
  const present = values.filter((v): v is number => v !== null && Number.isFinite(v));
  return present.length > 0 ? Math.max(...present) : null;
}