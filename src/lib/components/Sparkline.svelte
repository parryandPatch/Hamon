<script lang="ts">
  /**
   * A compact line/area chart drawn on a canvas.
   *
   * Canvas rather than SVG because a 180-sample sparkline redraws on every
   * tick: an SVG path per sample means a few hundred DOM attribute writes per
   * widget per second, which shows up as jank next to the numbers changing.
   *
   * `null` samples are drawn as a break in the line rather than being skipped,
   * so a metric that stops being reported visibly stops being plotted instead
   * of being interpolated over.
   */
  import { niceCeiling } from '../history.svelte';

  interface Props {
    values: (number | null)[];
    /** Fixed axis maximum, or `null` to auto-scale. */
    max?: number | null;
    /** Draw the area under the line. */
    fill?: boolean;
    /** Draw a dashed zero baseline. */
    zeroLine?: boolean;
    /** CSS colour for the line; `var(--x)` is resolved from the element. */
    stroke?: string;
    height?: number;
  }

  const {
    values,
    max = null,
    fill = true,
    zeroLine = false,
    stroke = 'var(--accent)',
    height = 46,
  }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  /** Bumped by the resize observer so a resize triggers a repaint. */
  let generation = $state(0);

  /** `false` when there is no data yet, so the canvas reads as "waiting". */
  const hasData = $derived(values.some((v) => v !== null));
  const ceiling = $derived(niceCeiling(peakOf(values), max));

  function peakOf(list: (number | null)[]): number | null {
    let peak: number | null = null;
    for (const v of list) {
      if (v === null) continue;
      peak = peak === null ? v : Math.max(peak, v);
    }
    return peak;
  }

  function lastPresentIndex(list: (number | null)[]): number {
    for (let i = list.length - 1; i >= 0; i--) {
      if (list[i] !== null) return i;
    }
    return -1;
  }

  /**
   * Resolves a colour to something a canvas will accept.
   *
   * Canvas does not understand `currentColor` or `color-mix()`, and its
   * `strokeStyle`/`fillStyle` parsers reject both. Reading the colour out of a
   * custom property is reliable where `getPropertyValue('var(--x)')` is not:
   * a custom property's computed value has its `var()` references already
   * substituted, so `--spark-color: var(--accent)` reads back as the accent's
   * hex. Alpha is then applied with `globalAlpha` rather than baked in.
   */
  function resolveColour(element: HTMLElement, property: string, fallback: string): string {
    const computed = getComputedStyle(element).getPropertyValue(property).trim();
    return computed === '' ? fallback : computed;
  }

  /**
   * Redraws on every change.
   *
   * Reading the element's box, resolving colours and resizing the backing store
   * all happen here, so a `$effect` keyed on the data plus a resize generation
   * covers both a new sample and a new width.
   */
  $effect(() => {
    void generation;
    /**
     * Tracked so a colour change repaints. The value itself is not used here —
     * it reaches the canvas through `--spark-color` and is read back off the
     * element, because canvas cannot resolve a CSS variable on its own.
     */
    void stroke;
    const element = canvas;
    if (!element) return;

    const list = values;
    const scale = ceiling;
    const data = hasData;
    const width = element.clientWidth;
    if (width === 0) return;

    const line = resolveColour(element, '--spark-color', '#4c9aff');
    const muted = resolveColour(element, '--muted', '#626b7d');

    const dpr = window.devicePixelRatio || 1;
    element.width = Math.round(width * dpr);
    element.height = Math.round(height * dpr);

    const ctx = element.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    ctx.globalAlpha = 1;

    const inset = 2;
    const plotHeight = height - inset * 2;
    const baseline = height - inset;
    const y = (value: number) => inset + plotHeight - (value / scale) * plotHeight;

    /**
     * A single sample sits at the right edge rather than the left, so a chart
     * that has only just started filling reads as "now", not "then".
     */
    const step = list.length > 1 ? width / (list.length - 1) : 0;
    const x = (i: number) => (list.length > 1 ? i * step : width);

    if (zeroLine) {
      ctx.strokeStyle = muted;
      ctx.globalAlpha = 0.25;
      ctx.lineWidth = 1;
      ctx.setLineDash([2, 3]);
      ctx.beginPath();
      ctx.moveTo(0, baseline);
      ctx.lineTo(width, baseline);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    if (!data) {
      // A flat dashed baseline reads as "waiting for data", which is honest; an
      // empty box reads as broken.
      ctx.strokeStyle = muted;
      ctx.globalAlpha = 0.3;
      ctx.lineWidth = 1;
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(0, baseline);
      ctx.lineTo(width, baseline);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
      return;
    }

    if (fill) {
      /**
       * Built by walking the same segments as the stroke, so a gap in the data
       * also breaks the fill instead of filling the hole.
       */
      ctx.beginPath();
      let started = false;
      for (let i = 0; i < list.length; i++) {
        const v = list[i];
        if (v === null) {
          started = false;
          continue;
        }
        if (!started) {
          ctx.moveTo(x(i), baseline);
          ctx.lineTo(x(i), y(v));
          started = true;
        } else {
          ctx.lineTo(x(i), y(v));
        }
      }
      const end = lastPresentIndex(list);
      if (end >= 0) {
        ctx.lineTo(x(end), baseline);
        ctx.closePath();
      }
      const gradient = ctx.createLinearGradient(0, inset, 0, height);
      gradient.addColorStop(0, line);
      gradient.addColorStop(1, 'transparent');
      ctx.fillStyle = gradient;
      ctx.globalAlpha = 0.3;
      ctx.fill();
      ctx.globalAlpha = 1;
    }

    ctx.strokeStyle = line;
    ctx.lineWidth = 1.5;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.beginPath();
    let drawing = false;
    for (let i = 0; i < list.length; i++) {
      const v = list[i];
      if (v === null) {
        drawing = false;
        continue;
      }
      if (!drawing) {
        ctx.moveTo(x(i), y(v));
        drawing = true;
      } else {
        ctx.lineTo(x(i), y(v));
      }
    }
    ctx.stroke();
  });

  /**
   * The canvas has no intrinsic size of its own — it fills its card — so a
   * window resize changes nothing reactive but changes everything visible.
   */
  $effect(() => {
    const element = canvas;
    if (!element || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => generation++);
    observer.observe(element);
    return () => observer.disconnect();
  });
</script>

<canvas bind:this={canvas} style:height="{height}px" style:--spark-color={stroke} aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
    width: 100%;
  }
</style>
