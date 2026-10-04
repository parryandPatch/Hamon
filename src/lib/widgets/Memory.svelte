<script lang="ts">
  /**
   * The `memory` widget.
   *
   * Physical and swap are separate widgets' worth of information, so both are
   * shown — but swap collapses to a single line when there is none, because on
   * a machine without swap an empty section is just noise.
   */
  import Bar from '../components/Bar.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, percent } from '../format';

  const memory = $derived(dashboard.snapshot?.memory ?? null);
  const hasSwap = $derived(memory !== null && memory.swap_total_bytes > 0);
</script>

{#if !memory}
  <Empty reason="Waiting for the first sample…" />
{:else if memory.total_bytes === 0}
  <Empty reason="No memory information" />
{:else}
  <div class="primary">
    <span class="value">{percent(memory.percent)}</span>
    <span class="of">{bytes(memory.used_bytes)} of {bytes(memory.total_bytes)}</span>
  </div>

  <Bar value={memory.percent} max={100} label="Physical" detail={percent(memory.percent)} />

  <div class="stats">
    <StatRow label="Available" value={bytes(memory.available_bytes)} />
    <StatRow label="Cached" value={bytes(memory.cached_bytes)} tone="muted" />
    <StatRow label="Buffers" value={bytes(memory.buffers_bytes)} tone="muted" />
  </div>

  {#if hasSwap}
    <div class="swap">
      <Bar
        value={memory.swap_used_bytes > 0 ? memory.swap_percent : 0}
        max={100}
        label="Swap"
        detail="{bytes(memory.swap_used_bytes)} / {bytes(memory.swap_total_bytes)}"
        tone={memory.swap_percent >= 50 ? 'warn' : 'accent'}
      />
    </div>
  {/if}
{/if}

<style>
  .primary {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .value {
    font-size: 1.55rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .of {
    font-size: 0.76rem;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .stats {
    display: grid;
    gap: 4px;
  }

  .swap {
    margin-top: 2px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
</style>