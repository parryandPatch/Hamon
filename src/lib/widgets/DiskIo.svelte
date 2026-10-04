<script lang="ts">
  /**
   * The `disk-io` widget: read and write over a user-chosen window.
   *
   * Functionally a two-series `history` widget. It exists as a separate kind
   * because disk throughput is the one metric where the *short* window is
   * misleading: a 1 Hz sparkline of a spinning disk looks like noise, and the
   * number people want is sustained throughput over tens of seconds.
   */
  import Sparkline from '../components/Sparkline.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, rate } from '../format';
  import { niceCeiling } from '../history.svelte';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const spanSeconds = $derived(Math.max(10, Number(config.span_seconds ?? 60)));
  const io = $derived(dashboard.snapshot?.disk_io ?? null);

  /** How many ticks fit the window at the live interval. */
  const sampleCount = $derived(Math.ceil((spanSeconds * 1000) / Math.max(1, dashboard.intervalMs)));

  function samplesFor(metric: string): (number | null)[] {
    const series = dashboard.history.get(metric);
    return series ? series.tail(sampleCount) : [];
  }

  const read = $derived(samplesFor('disk-read'));
  const write = $derived(samplesFor('disk-write'));

  /** One axis for both directions, so the comparison is honest. */
  const ceiling = $derived(
    niceCeiling(
      Math.max(...read.filter((v): v is number => v !== null), ...write.filter((v): v is number => v !== null)),
      null,
    ),
  );

  const mean = $derived.by(() => {
    const average = (list: (number | null)[]) => {
      const present = list.filter((v): v is number => v !== null);
      if (present.length === 0) return null;
      return present.reduce((a, b) => a + b, 0) / present.length;
    };
    return { read: average(read), write: average(write) };
  });
</script>

{#if !io}
  <Empty reason="Waiting for the first sample…" />
{:else}
  <div class="charts">
    <div class="chart">
      <div class="head">
        <span class="label">Read</span>
        <span class="value">{rate(io.read_bytes_per_sec)}</span>
      </div>
      <Sparkline values={read} max={ceiling} stroke="var(--accent)" height={40} zeroLine />
    </div>
    <div class="chart">
      <div class="head">
        <span class="label">Write</span>
        <span class="value">{rate(io.write_bytes_per_sec)}</span>
      </div>
      <Sparkline values={write} max={ceiling} stroke="var(--up)" height={40} zeroLine />
    </div>
  </div>

  <div class="axis">
    <span>0</span>
    <span class="ceiling">{rate(ceiling, 0)}</span>
  </div>

  <div class="stats">
    <StatRow label="Mean read ({spanSeconds}s)" value={rate(mean.read)} tone="muted" />
    <StatRow label="Mean write ({spanSeconds}s)" value={rate(mean.write)} tone="muted" />
    <StatRow label="Since boot" value="{bytes(io.read_bytes_total)} / {bytes(io.write_bytes_total)}" tone="muted" />
  </div>
{/if}

<style>
  .charts {
    display: grid;
    gap: 8px;
    flex: 1;
    align-content: center;
    min-height: 0;
  }

  .chart {
    display: grid;
    gap: 2px;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .label {
    font-size: 0.73rem;
    color: var(--text-dim);
  }

  .value {
    font-size: 0.95rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .axis {
    display: flex;
    justify-content: space-between;
    font-size: 0.64rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .stats {
    display: grid;
    gap: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
</style>
