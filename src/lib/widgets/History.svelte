<script lang="ts">
  /**
   * The `history` widget: a filled area chart of one metric.
   *
   * The window is measured in seconds and converted to a sample count from the
   * live sampling interval, so changing the interval keeps the chart's
   * time axis honest rather than silently changing what "60 seconds" means.
   */
  import Sparkline from '../components/Sparkline.svelte';
  import Empty from '../components/Empty.svelte';
  import { dashboard } from '../state.svelte';
  import { METRICS, readMetric, type MetricId } from '../metrics';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const metric = $derived((config.metric as MetricId) ?? 'cpu-usage');
  const spec = $derived(METRICS[metric] ?? METRICS['cpu-usage']);
  const spanSeconds = $derived(Math.max(10, Number(config.span_seconds ?? 60)));
  const value = $derived(readMetric(dashboard.snapshot, metric));
  const history = $derived(dashboard.history.get(metric));

  /** How many ticks fit in the window at the current interval. */
  const sampleCount = $derived(Math.ceil((spanSeconds * 1000) / Math.max(1, dashboard.intervalMs)));
  const samples = $derived(history ? history.tail(sampleCount) : []);

  const peak = $derived.by(() => {
    let p: number | null = null;
    for (const v of samples) {
      if (v === null) continue;
      p = p === null ? v : Math.max(p, v);
    }
    return p;
  });
</script>

{#if dashboard.snapshot === null}
  <Empty reason="Waiting for the first sample…" />
{:else if value === null}
  <Empty reason="Not available on this machine" hint="{spec.label} was not reported by any collector." />
{:else}
  <div class="chart">
    <Sparkline values={samples} max={spec.max} zeroLine={spec.max === null} height={82} />
  </div>
  <div class="legend">
    <span class="now">{spec.format(value)}</span>
    <span class="meta">peak {peak === null ? '—' : spec.format(peak)} · {spanSeconds}s</span>
  </div>
{/if}

<style>
  .chart {
    flex: 1;
    display: flex;
    align-items: center;
    min-height: 0;
  }

  .legend {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .now {
    font-size: 1.02rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .meta {
    font-size: 0.72rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>