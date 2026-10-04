<script lang="ts">
  /**
   * A key/value row for the small numeric readouts inside a widget.
   *
   * `label` is allowed to be `null` for a row that is only a value, which
   * happens where the label is already in a column heading.
   */
  import { DASH } from '../format';

  interface Props {
    label?: string | null;
    value: string | number | null;
    /** Rendered after the value, e.g. `cores`, `°C`. */
    unit?: string;
    /** Trailing parenthetical, e.g. a power limit next to the live draw. */
    note?: string | null;
    tone?: 'default' | 'muted' | 'accent' | 'danger' | 'warn';
    title?: string;
  }

  const { label = null, value, unit = '', note = null, tone = 'default', title }: Props = $props();

  const isEmpty = $derived(value === null || value === undefined || value === '');
</script>

<div class="row">
  {#if label}<span class="label">{label}</span>{/if}
  <span class="value tone-{tone}" {title}>
    {#if isEmpty}{DASH}{:else}{value}{/if}{#if unit && !isEmpty}<span class="unit">{unit}</span>{/if}
    {#if note && !isEmpty}<span class="note">{note}</span>{/if}
  </span>
</div>

<style>
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .label {
    font-size: 0.78rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value {
    font-size: 0.84rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .unit {
    color: var(--muted);
    margin-left: 3px;
    font-size: 0.76em;
  }

  .note {
    color: var(--muted);
    margin-left: 6px;
    font-size: 0.76em;
  }

  .tone-muted {
    color: var(--text-dim);
  }

  .tone-accent {
    color: var(--accent);
  }

  .tone-warn {
    color: var(--warn);
  }

  .tone-danger {
    color: var(--danger);
  }
</style>