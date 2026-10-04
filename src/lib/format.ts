/**
 * Number and duration formatting.
 *
 * Two rules run through all of these:
 *
 * 1. A value the backend could not measure formats as an em dash, never as
 *    `0`. `Snapshot` uses `null` to mean "unavailable on this platform" and
 *    conflating that with zero is the single easiest way to make a hardware
 *    monitor lie.
 * 2. Units scale to keep 3 significant digits, so a widget reads the same at
 *    `1.2 GB/s` and `840 MB/s` without the reader having to move the decimal.
 */

export const DASH = '—';

/** Bytes as a binary-prefixed size, e.g. `1.42 GiB`. */
export function bytes(value: number | null | undefined, digits = 2): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  if (value < 0) return '-' + bytes(-value, digits);
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
  let scaled = value;
  let unit = 0;
  while (scaled >= 1024 && unit < units.length - 1) {
    scaled /= 1024;
    unit += 1;
  }
  // Whole bytes never need a decimal point.
  return `${unit === 0 ? scaled : scaled.toFixed(digits)} ${units[unit]}`;
}

/** Bytes per second, e.g. `1.42 MiB/s`. */
export function rate(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  const magnitude = Math.abs(value);
  if (magnitude === 0) return '0 B/s';
  if (magnitude < 1) return `${value.toFixed(2)} B/s`;
  if (magnitude < 1024) return `${value.toFixed(0)} B/s`;
  return `${bytes(value, digits)}/s`;
}

/** Packets per second, abbreviated once the count gets large. */
export function packets(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  const magnitude = Math.abs(value);
  if (magnitude >= 1e9) return `${(value / 1e9).toFixed(1)} Gpkt/s`;
  if (magnitude >= 1e6) return `${(value / 1e6).toFixed(1)} Mpkt/s`;
  if (magnitude >= 1e3) return `${(value / 1e3).toFixed(1)} kpkt/s`;
  return `${value.toFixed(0)} pkt/s`;
}

/** A 0-100 percentage, or the dash when unavailable. */
export function percent(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  return `${value.toFixed(digits)}%`;
}

/** Degrees Celsius with one decimal. */
export function celsius(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  return `${value.toFixed(1)} °C`;
}

/** Megahertz, switching to GHz past 1000 MHz the way CPU marketing does. */
export function megahertz(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  if (value >= 1000) return `${(value / 1000).toFixed(2)} GHz`;
  return `${value.toFixed(value < 100 ? 1 : 0)} MHz`;
}

/** Watts, dropping to milliwatts below 1 W so idle GPUs still show a number. */
export function watts(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  if (value >= 1) return `${value.toFixed(1)} W`;
  return `${(value * 1000).toFixed(0)} mW`;
}

/** Fan RPM, or a fan percentage for devices that only report one. */
export function rpm(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  return `${Math.round(value).toLocaleString()} RPM`;
}

/** Volts from millivolts, used for battery voltage. */
export function millivolts(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  return `${(value / 1000).toFixed(2)} V`;
}

/** A plain count with thin-space thousands separators. */
export function count(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return DASH;
  return Math.round(value).toLocaleString();
}

/**
 * A duration in seconds as `3d 04:17` / `04:17` / `17s`.
 *
 * Used for uptime, where showing days only when there are some keeps the
 * common case short.
 */
export function duration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) return DASH;
  const total = Math.max(0, Math.floor(seconds));
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const clock = [hours, minutes, secs].map((n) => String(n).padStart(2, '0')).join(':');
  return days > 0 ? `${days}d ${clock}` : clock;
}

/** Battery remaining/charging time as `2h 15m`. */
export function minutesToClock(minutes: number | null | undefined): string {
  if (minutes === null || minutes === undefined || !Number.isFinite(minutes)) return DASH;
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** `Date`-friendly rendering of the backend's unix-seconds boot time. */
export function bootTime(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return DASH;
  const d = new Date(unixSeconds * 1000);
  if (Number.isNaN(d.getTime())) return DASH;
  return d.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}