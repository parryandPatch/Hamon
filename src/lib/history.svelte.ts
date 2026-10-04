/**
 * Fixed-length sample buffers for sparklines.
 *
 * Charts need history, and the sampler only delivers the current value. Two
 * constraints shape this:
 *
 * - History must survive a layout edit. Widgets are addressed by `kind` plus
 *   config, not by position, because positions shift the moment the user
 *   reorders anything.
 * - It must not grow without bound. A fixed-capacity buffer per series gives
 *   O(1) appends and a fixed footprint: 180 samples per series at one hertz is
 *   three minutes, and for a 10 Hz interval thirty seconds, which is the
 *   shortest window any widget offers.
 *
 * A `null` sample is *kept*, not skipped: a gap where the sensor read nothing
 * has to show as a gap, or the chart would interpolate across it and invent
 * data.
 *
 * This is a `.svelte.ts` module because the buffers are reactive. A widget
 * renders `series.toArray()` inside a `$derived`, and the sampler appends from
 * an event callback; a plain array would update the chart only when some
 * unrelated piece of state changed.
 */

export const CAPACITY = 180;

/** One metric's rolling window. */
export class Series {
  #values = $state<(number | null)[]>([]);
  readonly capacity: number;

  constructor(capacity = CAPACITY) {
    this.capacity = capacity;
  }

  get length(): number {
    return this.#values.length;
  }

  get full(): boolean {
    return this.#values.length >= this.capacity;
  }

  /** Appends one reading, evicting the oldest once the buffer is full. */
  push(value: number | null): void {
    if (this.#values.length >= this.capacity) {
      // `slice` rather than `shift`: `shift` is O(n) and this runs once per
      // metric per tick, forever.
      this.#values = [...this.#values.slice(1), value];
    } else {
      this.#values = [...this.#values, value];
    }
  }

  /** Oldest-to-newest, which is the order a chart's x-axis wants. */
  toArray(): (number | null)[] {
    return this.#values;
  }

  get latest(): number | null {
    const last = this.#values[this.#values.length - 1];
    return last === undefined ? null : last;
  }

  /** Largest non-null value, used to pick an automatic chart axis. */
  get peak(): number | null {
    let peak: number | null = null;
    for (const v of this.#values) {
      if (v === null) continue;
      peak = peak === null ? v : Math.max(peak, v);
    }
    return peak;
  }

  /** The trailing `count` samples, oldest first. */
  tail(count: number): (number | null)[] {
    return count >= this.#values.length ? this.#values : this.#values.slice(this.#values.length - count);
  }

  clear(): void {
    this.#values = [];
  }
}

/**
 * Every series the dashboard currently needs, keyed by metric id.
 *
 * The set is rebuilt whenever the layout changes: removing the last widget that
 * wanted a metric drops that metric's buffer rather than leaving it to be
 * sampled forever for nobody.
 */
export class History {
  /** Plain `Map` — the buffers inside are reactive on their own. */
  #series = new Map<string, Series>();

  /**
   * Bumped whenever the key set changes.
   *
   * Without it, a widget whose metric was just added would keep the `Series`
   * handle it looked up before, and its chart would silently stay empty. The
   * lookup reads this, so every `$derived` holding a `Series` re-runs.
   */
  #generation = $state(0);

  /** Declares which metrics the current layout needs. */
  retain(metrics: Iterable<string>): void {
    const next = [...new Set(metrics)];
    let changed = next.length !== this.#series.size;
    if (!changed) {
      for (const key of next) {
        if (!this.#series.has(key)) {
          changed = true;
          break;
        }
      }
    }
    if (!changed) return;

    for (const key of next) {
      if (!this.#series.has(key)) this.#series.set(key, new Series());
    }
    for (const key of [...this.#series.keys()]) {
      if (!next.includes(key)) this.#series.delete(key);
    }
    this.#generation++;
  }

  /** Records one reading for every wanted metric, from the current values. */
  record(read: (metric: string) => number | null): void {
    // Read the generation so this method is only ever reached through a
    // reactive dependency, which keeps `retain` and `record` consistent even if
    // a layout edit lands between two ticks.
    void this.#generation;
    for (const key of this.#series.keys()) {
      this.#series.get(key)?.push(read(key));
    }
  }

  get(metric: string): Series | undefined {
    void this.#generation;
    return this.#series.get(metric);
  }

  /** Clears every buffer, used when the layout is reset. */
  clear(): void {
    for (const s of this.#series.values()) s.clear();
  }
}

/**
 * A chart axis that is stable while the data is flat and grows only when the
 * data actually reaches it.
 *
 * Auto-scaling to the current maximum makes an idle CPU graph a violent
 * mountain range; growing in fixed steps instead keeps a steady line steady and
 * still accommodates a spike.
 */
export function niceCeiling(peak: number | null, fixedMax: number | null): number {
  if (fixedMax !== null) return fixedMax;
  if (peak === null || peak <= 0) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(peak));
  for (const step of [1, 1.25, 1.5, 2, 2.5, 3, 4, 5, 7.5, 10]) {
    if (peak <= step * magnitude) return step * magnitude;
  }
  return 10 * magnitude;
}
