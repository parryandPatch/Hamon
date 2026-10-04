/**
 * Dashboard state.
 *
 * A single module-level rune object rather than a context or a store library:
 * there is exactly one dashboard, one sampler, and one layout, and every widget
 * needs some slice of them. Sharing the state directly keeps the data flow
 * obvious — the sampler writes `snapshot`, the layout commands write `layout`,
 * and widgets read both.
 *
 * Layout edits follow one rule: the backend returns the authoritative layout,
 * and that return value replaces local state wholesale. Re-deriving the new
 * layout client-side would be a second implementation of the same mutation, and
 * the two would eventually disagree about clamping.
 */

import * as api from './tauri';
import { History } from './history.svelte';
import { readMetric } from './metrics';
import { catalogEntry, catalogKinds } from './catalog';
import type { Bootstrap, Layout, Snapshot } from './types';

class Dashboard {
  /** Latest sampler output. `null` until the first tick lands. */
  snapshot = $state<Snapshot | null>(null);

  /** Current layout, as the backend last reported it. */
  layout = $state<Layout>({
    version: 1,
    widgets: [],
    columns: 2,
    sample_interval_ms: 1000,
  });

  running = $state(true);
  intervalMs = $state(1000);
  bounds = $state({ min_ms: 250, max_ms: 5000 });

  /** True while the dashboard is in edit mode: widgets gain handles. */
  editing = $state(false);
  /** Index of the widget whose configuration panel is open, or `null`. */
  configuring = $state<number | null>(null);
  /** Index being dragged, for the reorder affordance. */
  dragging = $state<number | null>(null);

  /** Transient user-facing message, e.g. a failed save. */
  notice = $state<string | null>(null);

  /** True until the first `bootstrap` resolves, so the UI can hold a splash. */
  ready = $state(false);

  /** Rolling history for every chartable metric the layout needs. */
  readonly history = new History();

  #unlisten: (() => void) | null = null;
  #noticeTimer: ReturnType<typeof setTimeout> | null = null;
  #onFocus: (() => void) | null = null;
  #starting = false;

  async start(): Promise<void> {
    // Idempotent: the component body calls this on mount, and under Vite's HMR
    // that can happen more than once for one window. A second subscription
    // would double every chart's sample rate and leak the first listener.
    if (this.#starting || this.ready) return;
    this.#starting = true;
    try {
      this.#unlisten = await api.onSnapshot((snapshot) => this.#accept(snapshot));
      const boot: Bootstrap = await api.bootstrap();
      this.layout = boot.layout;
      this.running = boot.status.running;
      this.intervalMs = boot.status.interval_ms;
      this.bounds = boot.bounds;
      this.ready = true;
      this.#syncHistory();
    } catch (error) {
      this.#fail('could not reach the backend', error);
      this.ready = true;
    } finally {
      this.#starting = false;
    }

    // A window that has been in the background is showing stale numbers on
    // return; ask for a fresh tick rather than waiting out the interval. Held as
    // a field so `stop` can take it back off.
    this.#onFocus = () => void api.refresh();
    window.addEventListener('focus', this.#onFocus);
  }

  stop(): void {
    this.#unlisten?.();
    this.#unlisten = null;
    if (this.#onFocus) {
      window.removeEventListener('focus', this.#onFocus);
      this.#onFocus = null;
    }
    if (this.#noticeTimer !== null) {
      clearTimeout(this.#noticeTimer);
      this.#noticeTimer = null;
    }
  }

  #accept(snapshot: Snapshot): void {
    this.snapshot = snapshot;
    this.history.record((metric) => readMetric(snapshot, metric));
  }

  /**
   * Rebuilds the history key set from the widgets the layout contains.
   *
   * Which metrics need history is a property of the catalog, not of this file:
   * a widget declares its `series` there, plus a `metric` in its config if the
   * user picked one. Widgets the layout no longer contains stop being
   * sampled, so removing a chart also stops its sampling cost.
   */
  #syncHistory(): void {
    const wanted: string[] = [];
    for (const widget of this.layout.widgets) {
      const entry = catalogEntry(widget.kind);
      wanted.push(...(entry?.series ?? []));
      const metric = (widget.config as Record<string, unknown> | null)?.metric;
      if (typeof metric === 'string') wanted.push(metric);
    }
    this.history.retain(wanted);
  }

  /** Runs a layout command and adopts whatever the backend returns. */
  async #mutate(action: () => Promise<Layout>): Promise<void> {
    try {
      this.layout = await action();
      this.#syncHistory();
    } catch (error) {
      this.#fail('could not save the layout', error);
      // Re-read so the UI snaps back to what is actually on disk.
      try {
        this.layout = await api.getLayout();
        this.#syncHistory();
      } catch {
        /* the original notice is already on screen */
      }
    }
  }

  addWidget(kind: string): void {
    if (!catalogKinds().includes(kind)) {
      this.setNotice(`Unknown widget kind: ${kind}`);
      return;
    }
    void this.#mutate(() => api.addWidget(kind));
  }

  removeWidget(index: number): void {
    if (this.configuring === index) this.configuring = null;
    void this.#mutate(() => api.removeWidget(index));
  }

  moveWidget(from: number, to: number): void {
    if (from === to) return;
    // `configuring` tracks a position, so it has to follow its widget.
    const openIndex = this.configuring;
    void this.#mutate(() => api.moveWidget(from, to)).then(() => {
      if (openIndex === null || this.editing) return;
      this.configuring = openIndex === from ? to : openIndex > from && openIndex <= to ? openIndex - 1 : openIndex < from && openIndex >= to ? openIndex + 1 : openIndex;
    });
  }

  configureWidget(index: number, config: unknown): void {
    void this.#mutate(() => api.configureWidget(index, config));
  }

  setColumns(columns: number): void {
    void this.#mutate(() => api.setLayout({ ...this.layout, columns }));
  }

  setInterval(intervalMs: number): void {
    const clamped = Math.min(this.bounds.max_ms, Math.max(this.bounds.min_ms, Math.round(intervalMs)));
    this.intervalMs = clamped;
    void this.#mutate(() => api.setSampleInterval(clamped));
  }

  resetLayout(): void {
    this.configuring = null;
    this.history.clear();
    void this.#mutate(() => api.resetLayout());
  }

  toggleRunning(): void {
    const next = !this.running;
    this.running = next;
    const action = next ? api.resumeSampling() : api.pauseSampling();
    void action.catch((error) => {
      this.running = !next;
      this.#fail('could not change the sampling state', error);
    });
  }

  setNotice(message: string | null): void {
    this.notice = message;
    if (this.#noticeTimer) clearTimeout(this.#noticeTimer);
    if (message !== null) {
      this.#noticeTimer = setTimeout(() => (this.notice = null), 6000);
    }
  }

  #fail(context: string, error: unknown): void {
    const detail = error instanceof Error ? error.message : String(error);
    console.error(`${context}:`, error);
    this.setNotice(`${context}: ${detail}`);
  }
}

export const dashboard = new Dashboard();