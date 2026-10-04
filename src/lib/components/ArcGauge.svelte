<script lang="ts">
  /**
   * A big number with a circular progress arc.
   *
   * The default shape for a `gauge` widget, where one metric is the entire
   * point of the card. The value is the largest thing in the card because it is
   * the thing being scanned for.
   *
   * When `max` is `null` the arc auto-scales to the metric's own units (MHz, W,
   * B/s), which is why the sweep is normalised against `max` rather than
   * assuming 0-100.
   */
  import { DASH } from '../format';

  interface Props {
    value: number | null;
    /** Formats the number; the caller supplies it so units stay consistent. */
    format: (value: number) => string;
    /** Unit suffix shown small under the number. */
    unit?: string;
    max?: number | null;
    caption?: string;
    /** Footnote, e.g. "peak 4.2 GHz". */
    note?: string;
  }

  const { value, format, unit = '', max = 100, caption, note }: Props = $props();

  const RADIUS = 46;
  const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

  /**
   * The full-scale value the arc represents.
   *
   * A `null` `max` means "no natural ceiling" — MHz, watts, bytes per second —
   * and the current value then sits at the top of the arc. Callers that have a
   * peak to scale against (the gauge widget does) pass it in, so the sweep
   * reflects the window rather than just this tick.
   */
  const scale = $derived(
    max !== null && max > 0 ? max : value !== null && value > 0 ? value : 1,
  );
  const fraction = $derived(value === null ? 0 : Math.min(1, Math.max(0, value / scale)));
  const dash = $derived(CIRCUMFERENCE * fraction);
  const danger = $derived(fraction >= 0.9);
</script>

<div class="gauge" class:danger>
  <svg viewBox="0 0 120 120" role="img" aria-label="{caption}: {value === null ? 'unavailable' : format(value)}">
    <circle class="track" cx="60" cy="60" r={RADIUS} />
    <circle
      class="sweep"
      cx="60"
      cy="60"
      r={RADIUS}
      stroke-dasharray="{dash} {CIRCUMFERENCE - dash}"
    />
  </svg>
  <div class="readout">
    <div class="value">{value === null ? DASH : format(value)}</div>
    {#if unit && value !== null}<div class="unit">{unit}</div>{/if}
    {#if caption}<div class="caption">{caption}</div>{/if}
    {#if note}<div class="note">{note}</div>{/if}
  </div>
</div>

<style>
  .gauge {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    max-height: 168px;
    margin-inline: auto;
    width: 100%;
    max-width: 168px;
  }

  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
    stroke-width: 8;
    stroke-linecap: round;
  }

  .track {
    stroke: var(--track);
  }

  .sweep {
    stroke: var(--accent);
    transition: stroke-dasharray 160ms ease-out;
  }

  .danger .sweep {
    stroke: var(--danger);
  }

  .readout {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    padding: 0 14%;
    text-align: center;
    min-width: 0;
  }

  .value {
    font-size: clamp(1.5rem, 4.4cqw, 2.05rem);
    font-weight: 600;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    line-height: 1.05;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .unit {
    font-size: 0.72rem;
    color: var(--text-dim);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .caption {
    margin-top: 6px;
    font-size: 0.76rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .note {
    font-size: 0.7rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
</style>