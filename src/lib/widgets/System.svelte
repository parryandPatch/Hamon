<script lang="ts">
  /**
   * The `system` widget: identity, platform, and privilege hints.
   *
   * This is the widget that answers "what machine is this", and it is where a
   * missing metric gets explained. If SMC temperatures are absent because Hamon
   * is not running as root, the hint says so here rather than leaving the
   * temperatures widget to look broken.
   *
   * `compact` shows the fields that differ between machines; `full` adds the
   * ones that are usually the same and only interesting once.
   */
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { count, duration } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const full = $derived(config.style === 'full');
  const system = $derived(dashboard.snapshot?.system ?? null);

  /** `/etc/os-release`-style name when known, else the kernel's own opinion. */
  const platform = $derived.by(() => {
    if (!system) return null;
    return system.distro || system.os_name || system.kernel;
  });

  const cpuLabel = $derived.by(() => {
    if (!system) return null;
    if (system.cores_physical === system.cores_logical) {
      return `${system.cores_physical} cores`;
    }
    return `${system.cores_physical} cores / ${system.cores_logical} threads`;
  });
</script>

{#if !system}
  <Empty reason="Waiting for the first sample…" />
{:else}
  <div class="identity">
    <span class="host" title={system.hostname}>{system.hostname}</span>
    <span class="platform">{platform}</span>
    {#if system.model}
      <span class="model" title={system.model}>{system.model}</span>
    {/if}
  </div>

  <div class="stats">
    <StatRow label="CPU" value={system.cpu_brand || '—'} title={system.cpu_brand} />
    {#if cpuLabel}
      <StatRow label="Topology" value={cpuLabel} tone="muted" />
    {/if}
    {#if system.gpu_names.length > 0}
      <StatRow label="GPU" value={system.gpu_names.join(', ')} tone="muted" title={system.gpu_names.join(', ')} />
    {/if}
    <StatRow label="Uptime" value={duration(system.uptime_seconds)} tone="muted" />
    <StatRow label="Processes" value={count(dashboard.snapshot?.cpu.process_count ?? 0)} tone="muted" />

    {#if full}
      <StatRow label="OS" value={[system.os_name, system.os_version].filter(Boolean).join(' ')} tone="muted" />
      <StatRow label="Kernel" value={system.kernel} tone="muted" />
      <StatRow label="Architecture" value={system.arch} tone="muted" />
      {#if system.cpu_vendor}
        <StatRow label="CPU vendor" value={system.cpu_vendor} tone="muted" />
      {/if}
    {/if}
  </div>

  {#if system.privileged_hint}
    <p class="hint">
      <svg viewBox="0 0 14 14" aria-hidden="true">
        <path d="M7 1.6 12.8 12H1.2z" />
        <path d="M7 5.6v3.1M7 10.1v.6" />
      </svg>
      <span>{system.privileged_hint}</span>
    </p>
  {/if}
{/if}

<style>
  .identity {
    display: grid;
    gap: 1px;
    min-width: 0;
  }

  .host {
    font-size: 1.02rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .platform {
    font-size: 0.8rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model {
    font-size: 0.72rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stats {
    display: grid;
    gap: 4px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
    min-width: 0;
  }

  .stats :global(.value) {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hint {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    padding: 7px 9px;
    border-radius: 7px;
    background: var(--warn-soft);
    color: var(--warn);
    font-size: 0.72rem;
    line-height: 1.4;
  }

  .hint svg {
    width: 12px;
    height: 12px;
    flex: none;
    margin-top: 1px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
