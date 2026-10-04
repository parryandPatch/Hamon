/**
 * The widget catalog: what the user can add, what each widget is called, and
 * what shape its `config` payload has.
 *
 * This file is the frontend half of a contract with the Rust side and the two
 * halves must not drift:
 *
 * - `kind` values must match `CATALOG_KINDS` in `src-tauri/src/layout.rs`,
 *   otherwise the backend has no default config for them.
 * - `defaults` must match `default_config_for` in the same file, so a freshly
 *   added widget renders sensibly before the user configures anything.
 *
 * The config schema is declared rather than hand-built per widget so the
 * configuration panel can be generated: one `<select>`, one `<input type=number>`
 * or `<textarea>`, rendered from the same description the defaults come from.
 */

import type { LayoutWidget } from './types';

/** A value a widget's config can hold. */
export type ConfigValue = string | number | boolean | string[] | null;

export type Config = Record<string, ConfigValue>;

type FieldKind = 'select' | 'number' | 'text' | 'boolean' | 'lines' | 'ascii';

export interface Field {
  key: string;
  label: string;
  kind: FieldKind;
  /** `select` only. `label` may be `auto` to mean "let the widget decide". */
  options?: { value: string; label: string }[];
  /** `number` only. */
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  help?: string;
}

export interface CatalogEntry {
  kind: string;
  label: string;
  /** One-line description shown in the picker. */
  summary: string;
  group: 'Metrics' | 'Hardware' | 'Storage & Network' | 'System' | 'Custom';
  /** Sparkline history this widget keeps, as metric keys. Empty = none. */
  series?: string[];
  fields: Field[];
  defaults: Config;
}

const AUTO = { value: 'auto', label: 'Automatic' };

export const CATALOG: CatalogEntry[] = [
  {
    kind: 'gauge',
    label: 'Gauge',
    summary: 'One headline number with an arc, e.g. CPU load.',
    group: 'Metrics',
    series: [],
    fields: [
      {
        key: 'metric',
        label: 'Metric',
        kind: 'select',
        options: [
          { value: 'cpu-usage', label: 'CPU usage' },
          { value: 'cpu-frequency', label: 'CPU frequency' },
          { value: 'cpu-temperature', label: 'CPU temperature' },
          { value: 'memory-usage', label: 'Memory usage' },
          { value: 'swap-usage', label: 'Swap usage' },
          { value: 'gpu-usage', label: 'GPU usage' },
          { value: 'gpu-temperature', label: 'GPU temperature' },
          { value: 'gpu-power', label: 'GPU power' },
          { value: 'gpu-memory', label: 'GPU memory' },
          { value: 'gpu-fan', label: 'GPU fan' },
          { value: 'net-rx', label: 'Network download' },
          { value: 'net-tx', label: 'Network upload' },
          { value: 'disk-read', label: 'Disk read' },
          { value: 'disk-write', label: 'Disk write' },
          { value: 'disk-usage', label: 'Disk usage' },
          { value: 'battery-level', label: 'Battery level' },
          { value: 'battery-power', label: 'Battery power' },
        ],
      },
    ],
    defaults: { metric: 'cpu-usage' },
  },
  {
    kind: 'history',
    label: 'History',
    summary: 'A filled area chart of one metric over time.',
    group: 'Metrics',
    series: [],
    fields: [
      {
        key: 'metric',
        label: 'Metric',
        kind: 'select',
        options: [
          { value: 'cpu-usage', label: 'CPU usage' },
          { value: 'memory-usage', label: 'Memory usage' },
          { value: 'gpu-usage', label: 'GPU usage' },
          { value: 'gpu-temperature', label: 'GPU temperature' },
          { value: 'net-rx', label: 'Network download' },
          { value: 'net-tx', label: 'Network upload' },
          { value: 'disk-read', label: 'Disk read' },
          { value: 'disk-write', label: 'Disk write' },
        ],
      },
      { key: 'span_seconds', label: 'Window', kind: 'number', min: 10, max: 3600, step: 10, unit: 's' },
    ],
    defaults: { metric: 'cpu-usage', span_seconds: 60 },
  },
  {
    kind: 'cpu-cores',
    label: 'CPU cores',
    summary: 'A bar per logical core plus frequency and load average.',
    group: 'Hardware',
    series: [],
    fields: [{ key: 'show_frequency', label: 'Show frequency per core', kind: 'boolean' }],
    defaults: { show_frequency: true },
  },
  {
    kind: 'memory',
    label: 'Memory',
    summary: 'Physical memory and swap, used and available.',
    group: 'Hardware',
    series: [],
    fields: [],
    defaults: {},
  },
  {
    kind: 'gpu',
    label: 'GPU',
    summary: 'One card per graphics device: load, memory, clocks, power.',
    group: 'Hardware',
    series: [],
    fields: [{ key: 'device', label: 'Device', kind: 'select', options: [AUTO] }],
    defaults: { device: 'auto' },
  },
  {
    kind: 'network',
    label: 'Network',
    summary: 'Total download and upload rate over every interface.',
    group: 'Storage & Network',
    series: ['net-rx', 'net-tx'],
    fields: [],
    defaults: {},
  },
  {
    kind: 'network-interfaces',
    label: 'Network interfaces',
    summary: 'A row per interface with its address and traffic.',
    group: 'Storage & Network',
    series: [],
    fields: [{ key: 'interface', label: 'Interface', kind: 'select', options: [AUTO] }],
    defaults: { interface: 'auto' },
  },
  {
    kind: 'disk',
    label: 'Disks',
    summary: 'Block-device throughput, with a sparkline per direction.',
    group: 'Storage & Network',
    series: ['disk-read', 'disk-write'],
    fields: [],
    defaults: {},
  },
  {
    kind: 'disk-filesystems',
    label: 'Filesystems',
    summary: 'A row per mounted filesystem with a usage bar.',
    group: 'Storage & Network',
    series: [],
    fields: [{ key: 'mount', label: 'Mount point', kind: 'select', options: [AUTO] }],
    defaults: { mount: 'auto' },
  },
  {
    kind: 'disk-io',
    label: 'Disk I/O history',
    summary: 'Read and write rates over a longer window.',
    group: 'Storage & Network',
    series: ['disk-read', 'disk-write'],
    fields: [{ key: 'span_seconds', label: 'Window', kind: 'number', min: 10, max: 3600, step: 10, unit: 's' }],
    defaults: { span_seconds: 60 },
  },
  {
    kind: 'temperatures',
    label: 'Temperatures',
    summary: 'Every sensor the platform exposes, hottest first.',
    group: 'Hardware',
    series: [],
    fields: [{ key: 'show_fans', label: 'Show fan speeds', kind: 'boolean' }],
    defaults: { show_fans: true },
  },
  {
    kind: 'processes',
    label: 'Processes',
    summary: 'The busiest processes, by CPU or by memory.',
    group: 'System',
    series: [],
    fields: [
      {
        key: 'sort',
        label: 'Rank by',
        kind: 'select',
        options: [
          { value: 'cpu', label: 'CPU' },
          { value: 'memory', label: 'Memory' },
        ],
      },
      { key: 'limit', label: 'Rows', kind: 'number', min: 1, max: 25, step: 1 },
    ],
    defaults: { limit: 10, sort: 'cpu' },
  },
  {
    kind: 'battery',
    label: 'Battery',
    summary: 'Charge, health, cycle count and time remaining.',
    group: 'Hardware',
    series: [],
    fields: [{ key: 'show_health', label: 'Show health and cycles', kind: 'boolean' }],
    defaults: { show_health: true },
  },
  {
    kind: 'system',
    label: 'System',
    summary: 'Hostname, OS, kernel, hardware and privilege hints.',
    group: 'System',
    series: [],
    fields: [
      {
        key: 'style',
        label: 'Detail',
        kind: 'select',
        options: [
          { value: 'compact', label: 'Compact' },
          { value: 'full', label: 'Full' },
        ],
      },
    ],
    defaults: { style: 'compact' },
  },
  {
    kind: 'uptime',
    label: 'Uptime',
    summary: 'How long the machine has been up, and since when.',
    group: 'System',
    series: [],
    fields: [{ key: 'show_boot_time', label: 'Show boot time', kind: 'boolean' }],
    defaults: { show_boot_time: false },
  },
  {
    kind: 'custom-text',
    label: 'Custom text',
    summary: 'Free-form lines you type in.',
    group: 'Custom',
    series: [],
    fields: [{ key: 'lines', label: 'Lines', kind: 'lines' }],
    defaults: { lines: ['Hamon'] },
  },
  {
    kind: 'custom-ascii',
    label: 'Custom ASCII',
    summary: 'Preformatted art, rendered in a monospace block.',
    group: 'Custom',
    series: [],
    fields: [{ key: 'art', label: 'Artwork', kind: 'ascii' }],
    defaults: { art: '' },
  },
];

const BY_KIND = new Map(CATALOG.map((entry) => [entry.kind, entry]));

/** The catalog entry for a kind, or `undefined` if the layout is stale. */
export function catalogEntry(kind: string): CatalogEntry | undefined {
  return BY_KIND.get(kind);
}

/** Falls back to the first catalog entry so an unknown kind still renders. */
export function entryFor(widget: LayoutWidget): CatalogEntry {
  return (
    BY_KIND.get(widget.kind) ?? {
      kind: widget.kind,
      label: widget.kind,
      summary: 'Unknown widget kind.',
      group: 'Custom',
      fields: [],
      defaults: {},
    }
  );
}

/**
 * Merges a stored config over the catalog defaults.
 *
 * The stored payload can be missing keys (an older layout), carry the wrong
 * type (hand-edited file), or be absent entirely. Coercing here means no
 * widget body has to defend against any of that.
 */
export function resolveConfig(widget: LayoutWidget): Config {
  const entry = BY_KIND.get(widget.kind);
  const stored = (widget.config ?? {}) as Record<string, unknown>;
  const out: Config = { ...(entry?.defaults ?? {}) };
  for (const field of entry?.fields ?? []) {
    const raw = stored[field.key];
    const coerced = coerce(raw, field);
    if (coerced !== undefined) out[field.key] = coerced;
  }
  return out;
}

function coerce(raw: unknown, field: Field): ConfigValue | undefined {
  if (raw === undefined || raw === null) return undefined;
  switch (field.kind) {
    case 'select': {
      const value = String(raw);
      // An unknown value would render as an unlabelled select; fall back.
      return field.options?.some((o) => o.value === value) ? value : undefined;
    }
    case 'number': {
      const value = Number(raw);
      if (!Number.isFinite(value)) return undefined;
      const min = field.min ?? Number.NEGATIVE_INFINITY;
      const max = field.max ?? Number.POSITIVE_INFINITY;
      return Math.min(max, Math.max(min, Math.round(value)));
    }
    case 'boolean':
      return Boolean(raw);
    case 'lines':
      if (Array.isArray(raw)) return raw.map((l) => String(l));
      return String(raw).split('\n');
    case 'ascii':
    case 'text':
      return String(raw);
  }
}

/** The catalog kinds, for asserting parity with the Rust `CATALOG_KINDS`. */
export function catalogKinds(): string[] {
  return CATALOG.map((e) => e.kind);
}

/**
 * A short description of a widget's current configuration, for its card header.
 *
 * Without this a dashboard of four metrics-bearing cards reads as four identical
 * "History" titles, and the only way to tell them apart is to open each one's
 * settings. Naming the metric in the header means the title bar is enough.
 *
 * Values still at their default are omitted: "Compact" on every system widget
 * says nothing, and the header should only spend its space on what was changed.
 */
export function describeConfig(entry: CatalogEntry, config: Config): string | null {
  const metric = config.metric;
  if (typeof metric === 'string' && metric !== entry.defaults.metric) {
    return METRIC_LABELS[metric] ?? metric;
  }

  const parts: string[] = [];
  for (const field of entry.fields) {
    const value = config[field.key];
    if (value === entry.defaults[field.key]) continue;
    if (field.kind === 'select' && typeof value === 'string') {
      parts.push(field.options?.find((o) => o.value === value)?.label ?? value);
    } else if (field.kind === 'number') {
      parts.push(`${field.label.toLowerCase()} ${value}${field.unit ?? ''}`);
    }
  }
  return parts.length > 0 ? parts.join(' · ') : null;
}

/** Mirrors `metrics.ts` `METRICS[].label`; kept here to avoid a cycle. */
const METRIC_LABELS: Record<string, string> = {
  'cpu-usage': 'CPU usage',
  'cpu-frequency': 'CPU frequency',
  'cpu-temperature': 'CPU temperature',
  'memory-usage': 'Memory',
  'swap-usage': 'Swap',
  'gpu-usage': 'GPU usage',
  'gpu-temperature': 'GPU temperature',
  'gpu-power': 'GPU power',
  'gpu-memory': 'GPU memory',
  'gpu-fan': 'GPU fan',
  'net-rx': 'Download',
  'net-tx': 'Upload',
  'disk-read': 'Disk read',
  'disk-write': 'Disk write',
  'disk-usage': 'Disk usage',
  'battery-level': 'Battery level',
  'battery-power': 'Battery power',
};