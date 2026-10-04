<script lang="ts">
  /**
   * The `disk` widget: aggregate block-device throughput.
   *
   * Mirrors the `network` widget's shape deliberately — two directions, two
   * sparklines, one shared axis. Someone reading a disk widget and someone
   * reading a network widget are asking the same question of different pipes.
   *
   * Per-device rates are listed underneath only when there is more than one
   * device: a single disk's own rate is already the total, so repeating it
   * would be noise.
   */
  import Sparkline from '../components/Sparkline.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { bytes, percent, rate } from '../format';

  const io = $derived(dashboard.snapshot?.disk_io ?? null);
  const filesystems = $derived(dashboard.snapshot?.disk.filesystems ?? []);

  const readHistory = $derived(dashboard.history.get('disk-read')?.toArray() ?? []);
  const writeHistory = $derived(dashboard.history.get('disk-write')?.toArray() ?? []);

  const sharedMax = $derived.by(() => {
    let peak = 0;
    for (const list of [readHistory, writeHistory]) {
      for (const v of list) {
        if (v !== null && v > peak) peak = v;
      }
    }
    return peak > 0 ? peak : 1024 * 1024;
  });

  /** Fill percent of the fullest filesystem, the number people actually want. */
  const fullest = $derived.by(() => {
    if (filesystems.length === 0) return null;
    return filesystems.reduce((worst, f) => (f.percent > worst.percent ? f : worst));
  });
</script>

{#if !io}
  <Empty reason="Waiting for the first sample…" />
{:else}
  <div class="directions">
    <div class="direction">
      <div class="head">
        <svg class="glyph read" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 2.6v8.2m0 0 3.2-3.2M8 10.8 4.8 7.6" />
        </svg>
        <span class="label">Read</span>
      </div>
      <span class="value">{rate(io.read_bytes_per_sec)}</span>
      <Sparkline values={readHistory} max={sharedMax} stroke="var(--accent)" height={34} />
    </div>

    <div class="direction">
      <div class="head">
        <svg class="glyph write" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 13.4V5.2m0 0 3.2 3.2M8 5.2 4.8 8.4" />
        </svg>
        <span class="label">Write</span>
      </div>
      <span class="value">{rate(io.write_bytes_per_sec)}</span>
      <Sparkline values={writeHistory} max={sharedMax} stroke="var(--up)" height={34} />
    </div>
  </div>

  <div class="stats">
    {#if io.busy_percent !== null}
      <StatRow
        label="Busy"
        value={percent(io.busy_percent)}
        tone={io.busy_percent >= 90 ? 'danger' : io.busy_percent >= 60 ? 'warn' : 'default'}
      />
    {/if}
    <StatRow label="Total read" value={bytes(io.read_bytes_total)} tone="muted" />
    <StatRow label="Total written" value={bytes(io.write_bytes_total)} tone="muted" />
    {#if fullest}
      <StatRow
        label="Fullest volume"
        value={percent(fullest.percent)}
        note={fullest.mount_point}
        tone={fullest.percent >= 90 ? 'danger' : 'default'}
      />
    {/if}
  </div>

  {#if io.devices.length > 1}
    <ul class="devices">
      {#each io.devices as device (device.name)}
        <li>
          <span class="device-name" title={device.model ?? device.name}>{device.model ?? device.name}</span>
          <span class="device-rates">
            <span class="read">↓ {rate(device.read_bytes_per_sec)}</span>
            <span class="write">↑ {rate(device.write_bytes_per_sec)}</span>
          </span>
        </li>
      {/each}
    </ul>
  {/if}
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

  .read {
    color: var(--accent);
  }

  .write {
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

  .devices {
    list-style: none;
    margin: 0;
    padding: 8px 0 0;
    display: grid;
    gap: 5px;
    border-top: 1px dashed var(--border);
  }

  li {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .device-name {
    font-size: 0.74rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device-rates {
    display: flex;
    gap: 8px;
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .device-rates .read {
    color: var(--accent);
  }

  .device-rates .write {
    color: var(--up);
  }
</style>
