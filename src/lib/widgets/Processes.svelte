<script lang="ts">
  /**
   * The `processes` widget: the busiest processes, by CPU or by memory.
   *
   * The CPU column is a bar because the interesting question is "which one is
   * eating the machine", and a bare percentage makes you do that comparison in
   * your head. Memory is a bar too, for the same reason.
   *
   * Note the bars share the top row's value as their scale, so the visual
   * ranking and the numbers cannot disagree.
   */
  import Empty from '../components/Empty.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, percent } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const sortBy = $derived(config.sort === 'memory' ? 'memory' : 'cpu');
  const limit = $derived(Math.min(25, Math.max(1, Number(config.limit ?? 10))));

  const processes = $derived(dashboard.snapshot?.processes ?? null);

  const rows = $derived.by(() => {
    const list = sortBy === 'memory' ? (processes?.top_memory ?? []) : (processes?.top_cpu ?? []);
    return list.slice(0, limit);
  });

  /** The largest value in the list, so the longest bar is full-width. */
  const scale = $derived.by(() => {
    if (rows.length === 0) return 1;
    const values = rows.map((p) => (sortBy === 'memory' ? p.memory_bytes : p.cpu_percent));
    const peak = Math.max(...values);
    // A flat column of zeroes would divide by zero; 1 keeps the bars empty
    // rather than NaN.
    return peak > 0 ? peak : 1;
  });

  const total = $derived(dashboard.snapshot?.cpu.process_count ?? 0);
</script>

{#if !processes}
  <Empty reason="Waiting for the first sample…" />
{:else if rows.length === 0}
  <Empty reason="No processes reported" />
{:else}
  <div class="head">
    <span class="col-name">Process</span>
    <span class="col-metric">{sortBy === 'memory' ? 'Memory' : 'CPU'}</span>
  </div>

  <ul class="list">
    {#each rows as proc (proc.pid)}
      {@const value = sortBy === 'memory' ? proc.memory_bytes : proc.cpu_percent}
      <li title="{proc.name} (pid {proc.pid}){proc.user ? ` — ${proc.user}` : ''}">
        <div class="name">
          <span class="pid">{proc.pid}</span>
          <span class="text">{proc.name}</span>
          {#if proc.user}<span class="user">{proc.user}</span>{/if}
        </div>
        <div class="measure">
          <div class="track" class:danger={sortBy === 'cpu' && value >= 90}>
            <div class="fill" style:width="{(100 * value) / scale}%"></div>
          </div>
          <span class="value">{sortBy === 'memory' ? bytes(proc.memory_bytes, 1) : percent(proc.cpu_percent)}</span>
        </div>
      </li>
    {/each}
  </ul>

  <p class="foot">{rows.length} of {total} processes</p>
{/if}

<style>
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    font-size: 0.64rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 5px;
    overflow-y: auto;
    min-height: 0;
    align-content: start;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .name {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    /* Slightly over half so the numeric column always has room, whatever the
       column width. */
    flex: 1 1 55%;
  }

  .pid {
    font-size: 0.66rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex: none;
    min-width: 2.4em;
    text-align: right;
  }

  .text {
    font-size: 0.79rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .user {
    font-size: 0.66rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .measure {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 0 1 45%;
    min-width: 0;
  }

  .track {
    flex: 1;
    height: 4px;
    min-width: 18px;
    border-radius: 999px;
    background: var(--track);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent-dim), var(--accent));
    transition: width 140ms ease-out;
  }

  .track.danger .fill {
    background: linear-gradient(90deg, var(--danger-dim), var(--danger));
  }

  .value {
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    min-width: 4.6em;
    text-align: right;
  }

  .foot {
    margin: 0;
    padding-top: 7px;
    border-top: 1px solid var(--border);
    font-size: 0.68rem;
    color: var(--muted);
  }
</style>
