<script lang="ts">
  /**
   * The `uptime` widget.
   *
   * Uptime in one big number, because that is the only thing this widget is
   * for. Boot time is behind the config toggle since it changes once a day and
   * is only interesting when something is wrong.
   *
   * The uptime value comes from the sampler, which computes it from
   * `boot_time` rather than tracking ticks — so it stays correct across a
   * sleep, which a client-side counter would not.
   */
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bootTime, duration } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const showBootTime = $derived(Boolean(config.show_boot_time));
  const system = $derived(dashboard.snapshot?.system ?? null);

  /** Split for display so days/hours can be sized differently. */
  const parts = $derived.by(() => {
    const seconds = system?.uptime_seconds ?? 0;
    const total = Math.max(0, Math.floor(seconds));
    return {
      days: Math.floor(total / 86400),
      clock: duration(seconds).includes('d ') ? duration(seconds).split(' ')[1] : duration(seconds),
    };
  });

  const cpuCount = $derived(system ? system.cores_logical : 0);
</script>

<div class="primary">
  <span class="value">{system ? duration(system.uptime_seconds) : '—'}</span>
  <span class="caption">up</span>
</div>

<div class="stats">
  {#if parts.days > 0}
    <StatRow label="Days" value={parts.days} tone="muted" />
  {/if}
  <StatRow label="Cores online" value={cpuCount} tone="muted" />
  {#if showBootTime}
    <StatRow label="Booted" value={bootTime(system?.boot_time)} tone="muted" />
  {/if}
</div>

<style>
  .primary {
    display: flex;
    align-items: baseline;
    gap: 7px;
    flex-wrap: wrap;
  }

  .value {
    font-size: 1.55rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .caption {
    font-size: 0.78rem;
    color: var(--text-dim);
  }

  .stats {
    display: grid;
    gap: 4px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
</style>
