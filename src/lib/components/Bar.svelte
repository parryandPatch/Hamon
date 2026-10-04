<script lang="ts">
  /**
   * A labelled usage bar.
   *
   * Used wherever a widget shows "how much of something is used": memory,
   * filesystems, battery, individual cores. The bar itself is decorative —
   * the value is always printed as text beside it, because a bar alone is not
   * readable to a screen reader and is imprecise for a number like 63.7%.
   */
  interface Props {
    value: number | null;
    label?: string;
    detail?: string;
    /** Full-scale value. Percentages pass 100; anything else its own max. */
    max?: number;
    /** Overrides the bar colour, e.g. red past 90%. */
    tone?: 'auto' | 'warn' | 'danger' | 'accent';
    /** Height of the filled track, in pixels. */
    thickness?: number;
  }

  const { value, label, detail, max = 100, tone = 'auto', thickness = 6 }: Props = $props();

  const fraction = $derived(value === null ? 0 : Math.min(1, Math.max(0, value / max)));
  const danger = $derived(value !== null && value / max >= 0.9);
  const resolvedTone = $derived(tone === 'auto' ? (danger ? 'danger' : 'accent') : tone);
</script>

<div class="bar" style:--thickness="{thickness}px">
  {#if label}
    <div class="head">
      <span class="label" title={label}>{label}</span>
      {#if detail}<span class="detail">{detail}</span>{/if}
    </div>
  {/if}
  <div
    class="track"
    role="meter"
    aria-valuenow={value === null ? undefined : Math.round(fraction * 100)}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-label={label}
  >
    <div class="fill tone-{resolvedTone}" style:width="{fraction * 100}%"></div>
  </div>
</div>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .label {
    font-size: 0.78rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail {
    font-size: 0.78rem;
    color: var(--text);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .track {
    height: var(--thickness);
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: 999px;
    /* Short transition so a one-tick jump does not look like an animation lag,
       but a real spike is still readable rather than snapping. */
    transition: width 140ms ease-out;
  }

  .tone-accent {
    background: linear-gradient(90deg, var(--accent-dim), var(--accent));
  }

  .tone-warn {
    background: linear-gradient(90deg, var(--warn-dim), var(--warn));
  }

  .tone-danger {
    background: linear-gradient(90deg, var(--danger-dim), var(--danger));
  }
</style>