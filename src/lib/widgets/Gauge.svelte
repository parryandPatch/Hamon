<script lang="ts">
  /**
   * The `gauge` widget: one metric, big.
   *
   * Two things beyond the arc earn their place. The value is also plotted for
   * the last few minutes, because a number that says "80%" tells you nothing
   * about whether that is a spike or the new normal. And the metric's own peak
   * is printed under it, which turns the chart's auto-scaling axis into
   * information rather than an arbitrary number.
   */
  import ArcGauge from '../components/ArcGauge.svelte';
  import Sparkline from '../components/Sparkline.svelte';
  import Empty from '../components/Empty.svelte';
  import { dashboard } from '../state.svelte';
  import { METRICS, readMetric, type MetricId } from '../metrics';
  import { DASH } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const metric = $derived((config.metric as MetricId) ?? 'cpu-usage');
  const spec = $derived(METRICS[metric] ?? METRICS['cpu-usage']);
  const value = $derived(readMetric(dashboard.snapshot, metric));
  const history = $derived(dashboard.history.get(metric));
  const peak = $derived(history?.peak ?? null);
  const ready = $derived(dashboard.snapshot !== null);

  /** A max of `null` means auto-scale; the arc then uses the peak. */
  const arcMax = $derived(spec.max ?? (peak !== null && peak > 0 ? peak : null));
</script>

{#if !ready}
  <Empty reason="Waiting for the first sample…" />
{:else if value === null}
  <Empty reason="Not available on this machine" hint="{spec.label} was not reported by any collector." />
{:else}
  <ArcGauge value={value} format={spec.format} max={arcMax} caption={spec.label} />
  <div class="chart">
    <Sparkline values={history?.toArray() ?? []} max={spec.max} zeroLine={spec.max === null} />
  </div>
  <!-- Only the peak: `spec.format` already carries the unit, so repeating it
       here would print "peak 2.25 GHz  MHz". -->
  <div class="foot">
    <span>peak {peak === null ? DASH : spec.format(peak)}</span>
    <span class="window">{history?.length ?? 0} samples</span>
  </div>
{/if}

<style>
  .chart {
    margin-top: 2px;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 0.7rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .window {
    white-space: nowrap;
  }
</style>