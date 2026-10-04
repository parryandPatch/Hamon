<script lang="ts">
  /**
   * The `disk-filesystems` widget: a row per mount with a usage bar.
   *
   * `used` and `free` are both shown rather than just the percentage because
   * the useful absolute numbers differ: "62% of 2 TiB" leaves out that 760 GiB
   * is still free, which is the number you act on.
   *
   * The filesystem type is shown as a dim suffix on the mount point — it is
   * rarely the interesting field but is what tells you why an APFS volume shows
   * up twice under `/System/Volumes/Data`.
   */
  import Bar from '../components/Bar.svelte';
  import Empty from '../components/Empty.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, percent } from '../format';
  import type { Config } from '../catalog';
  import type { Filesystem } from '../types';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const all = $derived(dashboard.snapshot?.disk.filesystems ?? []);

  const filesystems = $derived.by(() => {
    const wanted = String(config.mount ?? 'auto');
    return wanted === 'auto' ? all : all.filter((f) => f.mount_point === wanted);
  });

  const totals = $derived(
    filesystems.reduce(
      (acc, f) => ({
        total: acc.total + f.total_bytes,
        used: acc.used + f.used_bytes,
        free: acc.free + f.free_bytes,
      }),
      { total: 0, used: 0, free: 0 },
    ),
  );

  function toneFor(fs: Filesystem): 'auto' | 'warn' | 'danger' {
    // macOS reports APFS volumes sharing a container, so summing their capacity
    // double-counts. Scaling the thresholds by how full the *set* is keeps a
    // single "96% full" container from colouring five healthy volumes red.
    const overall = totals.total > 0 ? (100 * totals.used) / totals.total : 0;
    if (fs.percent >= 98 || overall >= 98) return 'danger';
    if (fs.percent >= 85 || overall >= 85) return 'warn';
    return 'auto';
  }
</script>

{#if all.length === 0}
  <Empty reason="Waiting for the first sample…" />
{:else if filesystems.length === 0}
  <Empty reason="No matching mount point" />
{:else}
  <ul class="list">
    {#each filesystems as fs (fs.mount_point)}
      <li>
        <Bar
          value={fs.percent}
          max={100}
          label={fs.mount_point}
          detail="{percent(fs.percent)} · {bytes(fs.free_bytes)} free"
          tone={toneFor(fs)}
        />
        <div class="sub">
          {bytes(fs.used_bytes)} used of {bytes(fs.total_bytes)}
          {#if fs.fs_type}<span class="fs-type">{fs.fs_type}</span>{/if}
          {#if fs.kind !== 'unknown'}<span class="fs-kind">{fs.kind}</span>{/if}
        </div>
      </li>
    {/each}
  </ul>

  {#if filesystems.length > 1}
    <div class="totals">
      <Bar
        value={totals.total > 0 ? (100 * totals.used) / totals.total : 0}
        max={100}
        label="All volumes"
        detail="{bytes(totals.free)} free"
      />
    </div>
  {/if}
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 11px;
    overflow-y: auto;
    min-height: 0;
  }

  li {
    display: grid;
    gap: 3px;
    min-width: 0;
  }

  .sub {
    font-size: 0.7rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    display: flex;
    gap: 7px;
    min-width: 0;
  }

  .fs-type,
  .fs-kind {
    color: var(--muted);
    opacity: 0.75;
    white-space: nowrap;
  }

  .fs-kind::before {
    content: '·';
    margin-right: 5px;
  }

  .totals {
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }
</style>
