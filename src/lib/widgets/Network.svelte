<script lang="ts">
  /**
   * The `network` widget: aggregate throughput.
   *
   * Download and upload are stacked rather than averaged, because the failure
   * mode people look for in a network widget is asymmetry — a saturated upload
   * shows up as an asymmetric bar long before it shows up as a slow download.
   *
   * Loopback is excluded from the totals by the backend, so the numbers here
   * are what actually left the machine.
   */
  import Sparkline from '../components/Sparkline.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, packets, rate } from '../format';

  const net = $derived(dashboard.snapshot?.network ?? null);

  const rxHistory = $derived(dashboard.history.get('net-rx')?.toArray() ?? []);
  const txHistory = $derived(dashboard.history.get('net-tx')?.toArray() ?? []);

  /**
   * One axis for both directions, or the smaller series would be flattened
   * against the larger one's scale and look idle.
   */
  const sharedMax = $derived.by(() => {
    let peak = 0;
    for (const list of [rxHistory, txHistory]) {
      for (const v of list) {
        if (v !== null && v > peak) peak = v;
      }
    }
    return peak > 0 ? peak : 1024;
  });

  const upInterfaces = $derived((net?.interfaces ?? []).filter((i) => i.is_up && !i.is_loopback).length);
</script>

{#if !net}
  <Empty reason="Waiting for the first sample…" />
{:else}
  <div class="directions">
    <div class="direction">
      <div class="head">
        <svg class="glyph down" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 2v9m0 0 3.6-3.6M8 11 4.4 7.4" />
        </svg>
        <span class="label">Download</span>
      </div>
      <span class="value">{rate(net.rx_bytes_per_sec)}</span>
      <Sparkline values={rxHistory} max={sharedMax} stroke="var(--accent)" height={34} />
    </div>

    <div class="direction">
      <div class="head">
        <svg class="glyph up" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 14V5m0 0 3.6 3.6M8 5 4.4 8.6" />
        </svg>
        <span class="label">Upload</span>
      </div>
      <span class="value">{rate(net.tx_bytes_per_sec)}</span>
      <Sparkline values={txHistory} max={sharedMax} stroke="var(--up)" height={34} />
    </div>
  </div>

  <div class="stats">
    <StatRow label="Packets" value="{packets(net.rx_packets_per_sec)} / {packets(net.tx_packets_per_sec)}" tone="muted" />
    <StatRow label="Session total" value="{bytes(net.total_rx_bytes)} / {bytes(net.total_tx_bytes)}" tone="muted" />
    <StatRow label="Interfaces up" value={upInterfaces} tone="muted" />
  </div>
{/if}

<style>
  .directions {
    display: grid;
    gap: 10px;
  }

  .direction {
    display: grid;
    gap: 3px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .glyph {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .down {
    color: var(--accent);
  }

  .up {
    color: var(--up);
  }

  .label {
    font-size: 0.73rem;
    color: var(--text-dim);
  }

  .value {
    font-size: 1.02rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }

  .stats {
    display: grid;
    gap: 4px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
</style>