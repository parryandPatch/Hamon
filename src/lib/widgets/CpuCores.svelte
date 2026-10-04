<script lang="ts">
  /**
   * The `cpu-cores` widget.
   *
   * A bar per logical core, which is the thing that actually answers "what is
   * my CPU doing" — an aggregate number hides one saturated core among nine idle
   * ones. Core bars are vertical, because cores are a column of peers; a
   * horizontal bar chart of cores would have to be read as 10 unrelated rows.
   */
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { megahertz, percent } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const showFrequency = $derived(Boolean(config.show_frequency));
  const cpu = $derived(dashboard.snapshot?.cpu ?? null);
  const cores = $derived(cpu?.per_core ?? []);
  const frequencies = $derived(cpu?.per_core_frequency_mhz ?? []);
</script>

{#if !cpu}
  <Empty reason="Waiting for the first sample…" />
{:else if cores.length === 0}
  <Empty reason="No per-core data" />
{:else}
  <div class="cores" class:with-freq={showFrequency}>
    {#each cores as usage, i (i)}
      <div class="core">
        <div class="column">
          <div
            class="bar"
            class:hot={usage >= 90}
            title="Core {i}: {percent(usage)}"
            role="meter"
            aria-valuenow={Math.round(usage)}
            aria-valuemin="0"
            aria-valuemax="100"
            aria-label="Core {i}"
          >
            <div class="fill" style:height="{Math.min(100, Math.max(0, usage))}%"></div>
          </div>
          {#if showFrequency}
            <span class="freq" class:dim={frequencies[i] === null}>
              {frequencies[i] === null ? '—' : Math.round(frequencies[i]!)}
            </span>
          {/if}
          <span class="index">{i}</span>
        </div>
      </div>
    {/each}
  </div>

  <div class="summary">
    <StatRow label="Total" value={percent(cpu.usage)} />
    <StatRow label="Frequency" value={megahertz(cpu.frequency_mhz)} />
    <StatRow
      label="Load average"
      value="{cpu.load1.toFixed(2)} {cpu.load5.toFixed(2)} {cpu.load15.toFixed(2)}"
      tone="muted"
    />
    <StatRow label="Threads" value={cpu.thread_count} tone="muted" />
  </div>
{/if}

<style>
  .cores {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13px, 1fr));
    gap: 3px;
    align-items: end;
    min-height: 58px;
  }

  .column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 0;
  }

  .bar {
    position: relative;
    width: 100%;
    min-width: 8px;
    /* Height fixed so the chart does not jump as values change. */
    height: 54px;
    border-radius: 3px;
    background: var(--track);
    overflow: hidden;
    display: flex;
    align-items: flex-end;
  }

  .fill {
    width: 100%;
    background: linear-gradient(180deg, var(--accent), var(--accent-dim));
    border-radius: 3px;
    transition: height 140ms ease-out;
  }

  .bar.hot .fill {
    background: linear-gradient(180deg, var(--danger), var(--danger-dim));
  }

  .freq {
    font-size: 0.6rem;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }

  .freq.dim {
    color: var(--muted);
  }

  .index {
    font-size: 0.58rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .summary {
    display: grid;
    gap: 4px;
    margin-top: 4px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
</style>