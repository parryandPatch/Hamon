<script lang="ts">
  /**
   * The `gpu` widget.
   *
   * Machines with two GPUs are common enough (integrated plus discrete, or two
   * cards) that every device gets its own block rather than only the busiest
   * one being shown.
   *
   * Which fields render depends entirely on what the driver exposes: on Linux
   * an amdgpu card reports VRAM and power while an Intel iGPU may report neither.
   * A field with no reading is omitted instead of rendered as a zero.
   */
  import Bar from '../components/Bar.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, celsius, megahertz, percent, rpm, watts } from '../format';
  import type { Config } from '../catalog';
  import type { GpuDevice } from '../types';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const devices = $derived.by(() => {
    const all = dashboard.snapshot?.gpu.devices ?? [];
    const wanted = String(config.device ?? 'auto');
    return wanted === 'auto' ? all : all.filter((d) => String(d.index) === wanted);
  });

  const ready = $derived(dashboard.snapshot !== null);

  function memoryFraction(d: GpuDevice): number | null {
    if (d.memory_used_bytes === null || !d.memory_total_bytes) return null;
    return (100 * d.memory_used_bytes) / d.memory_total_bytes;
  }
</script>

{#if !ready}
  <Empty reason="Waiting for the first sample…" />
{:else if devices.length === 0}
  <Empty
    reason="No graphics device detected"
    hint="Discrete GPUs often need vendor permissions; check the system widget for a privilege hint."
  />
{:else}
  {#each devices as device (device.index)}
    <div class="device">
      <div class="head">
        <span class="name" title={device.name}>{device.name}</span>
        <span class="vendor">{device.vendor}{device.kind ? ` · ${device.kind}` : ''}</span>
      </div>

      {#if device.usage_percent !== null}
        <Bar value={device.usage_percent} max={100} label="Load" detail={percent(device.usage_percent)} />
      {/if}

      {#if memoryFraction(device) !== null}
        <Bar
          value={memoryFraction(device)}
          max={100}
          label="VRAM"
          detail="{bytes(device.memory_used_bytes)} / {bytes(device.memory_total_bytes)}"
        />
      {/if}

      <div class="stats">
        {#if device.temperature !== null}
          <StatRow label="Temperature" value={celsius(device.temperature)} />
        {/if}
        {#if device.power_watts !== null}
          <StatRow
            label="Power"
            value={watts(device.power_watts)}
            note={device.power_limit_watts !== null ? `${watts(device.power_limit_watts)} limit` : null}
          />
        {/if}
        {#if device.fan_percent !== null}
          <StatRow label="Fan" value={percent(device.fan_percent)} />
        {/if}
        {#if device.clock_mhz !== null}
          <StatRow label="Core clock" value={megahertz(device.clock_mhz)} />
        {/if}
        {#if device.memory_clock_mhz !== null}
          <StatRow label="Memory clock" value={megahertz(device.memory_clock_mhz)} />
        {/if}
        {#if device.driver_version}
          <StatRow label="Driver" value={device.driver_version} tone="muted" />
        {/if}
      </div>

      {#if device.engines.length > 0}
        <div class="engines">
          {#each device.engines as [name, load] (name)}
            <Bar value={load} max={100} label={name} detail={percent(load)} thickness={4} />
          {/each}
        </div>
      {/if}
    </div>
  {/each}
{/if}

<style>
  .device + .device {
    margin-top: 4px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .name {
    font-size: 0.86rem;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .vendor {
    font-size: 0.7rem;
    color: var(--muted);
    white-space: nowrap;
  }

  .stats {
    display: grid;
    gap: 4px;
  }

  .engines {
    display: grid;
    gap: 5px;
    margin-top: 2px;
    padding-top: 8px;
    border-top: 1px dashed var(--border);
  }
</style>