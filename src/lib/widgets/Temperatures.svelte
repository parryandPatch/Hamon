<script lang="ts">
  /**
   * The `temperatures` widget: every sensor the platform exposes.
   *
   * Sorted hottest first, which is the only ordering that makes the widget
   * useful — the sensor you want is the one near the top.
   *
   * Sensors that need root to read are still listed, flagged, rather than
   * hidden. On Linux, `hwmon` is readable but `coretemp`/`k10temp` labels often
   * need privileges; a widget that silently shows two of six fans because it
   * could not open the others is worse than one that says so.
   */
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { celsius, millivolts, rpm, watts } from '../format';
  import type { Config } from '../catalog';
  import type { Sensor } from '../types';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const showFans = $derived(Boolean(config.show_fans));
  const sensors = $derived(dashboard.snapshot?.sensors.sensors ?? []);

  const visible = $derived.by(() => {
    const list = sensors.filter((s) => s.temp_c > 0 || s.rpm !== null || s.watts !== null || s.millivolts !== null);
    if (showFans) return list;
    // Fans still matter to thermal health, so with `showFans` off only the
    // fan-only entries are dropped — a sensor reporting both is kept.
    return list.filter((s) => !(s.rpm !== null && s.temp_c <= 0 && s.watts === null && s.millivolts === null));
  });

  const sorted = $derived(
    [...visible].sort((a, b) => {
      const aTemp = a.temp_c > 0 ? a.temp_c : Number.NEGATIVE_INFINITY;
      const bTemp = b.temp_c > 0 ? b.temp_c : Number.NEGATIVE_INFINITY;
      return bTemp - aTemp || a.label.localeCompare(b.label);
    }),
  );

  const hottest = $derived(sorted.find((s) => s.temp_c > 0) ?? null);
  const needsPrivileges = $derived(sorted.some((s) => s.needs_privileges));

  /** Celsius at which a reading is tinted: driver-reported limit, or a default. */
  function hot(s: Sensor): boolean {
    return s.temp_c > 0 && s.temp_c >= (s.high_c ?? 80);
  }
</script>

{#if !dashboard.snapshot}
  <Empty reason="Waiting for the first sample…" />
{:else if sorted.length === 0}
  <Empty
    reason="No sensors reported"
    hint="macOS exposes temperatures through SMC, which needs root; the system widget will say so."
  />
{:else}
  <ul class="list">
    {#each sorted as sensor (sensor.label + sensor.kind)}
      <li class:hot={hot(sensor)} class:locked={sensor.needs_privileges}>
        <span class="label" title="{sensor.kind}{sensor.high_c ? ` · high ${sensor.high_c}°C` : ''}{sensor.critical_c ? ` · critical ${sensor.critical_c}°C` : ''}">
          {#if sensor.needs_privileges}
            <svg class="lock" viewBox="0 0 12 12" aria-hidden="true">
              <rect x="2.4" y="5.2" width="7.2" height="5.4" rx="1.2" />
              <path d="M4.2 5.2V3.9a1.8 1.8 0 0 1 3.6 0v1.3" />
            </svg>
          {/if}
          {sensor.label}
        </span>
        <span class="value">
          {#if sensor.temp_c > 0}{celsius(sensor.temp_c)}{/if}
          {#if sensor.rpm !== null}<span class="extra">{rpm(sensor.rpm)}</span>{/if}
          {#if sensor.watts !== null}<span class="extra">{watts(sensor.watts)}</span>{/if}
          {#if sensor.millivolts !== null}<span class="extra">{millivolts(sensor.millivolts)}</span>{/if}
          {#if sensor.temp_c <= 0 && sensor.rpm === null && sensor.watts === null && sensor.millivolts === null}—{/if}
        </span>
      </li>
    {/each}
  </ul>

  {#if hottest || needsPrivileges}
    <div class="foot">
      {#if hottest}
        <StatRow label="Hottest" value={celsius(hottest.temp_c)} note={hottest.label} tone={hot(hottest) ? 'danger' : 'default'} />
      {/if}
      {#if needsPrivileges}
        <p class="note">Some sensors need elevated privileges. Run Hamon with <code>sudo</code> to read them all.</p>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 5px;
    overflow-y: auto;
    min-height: 0;
  }

  li {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.78rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lock {
    width: 10px;
    height: 10px;
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    color: var(--warn);
  }

  .value {
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .extra {
    color: var(--muted);
    margin-left: 8px;
    font-size: 0.86em;
  }

  li.hot .value {
    color: var(--danger);
  }

  li.locked .label {
    opacity: 0.85;
  }

  .foot {
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .note {
    margin: 6px 0 0;
    font-size: 0.7rem;
    color: var(--warn);
    line-height: 1.35;
  }

  code {
    font-family: var(--mono);
    font-size: 0.95em;
    background: var(--hover);
    padding: 1px 4px;
    border-radius: 4px;
  }
</style>
