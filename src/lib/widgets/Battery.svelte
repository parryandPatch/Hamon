<script lang="ts">
  /**
   * The `battery` widget.
   *
   * A desktop has no battery, and saying so plainly is better than showing a
   * card of em dashes — hence the single, specific empty state.
   *
   * Charge is the headline because it is the only number anyone checks, and the
   * colour of the bar carries meaning: it goes amber below 20% and below 10%
   * minutes of estimated runtime, which is when plugging in actually matters.
   */
  import Bar from '../components/Bar.svelte';
  import Empty from '../components/Empty.svelte';
  import StatRow from '../components/StatRow.svelte';
  import { dashboard } from '../state.svelte';
  import { celsius, millivolts, minutesToClock, percent, watts } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const showHealth = $derived(Boolean(config.show_health));
  const battery = $derived(dashboard.snapshot?.battery ?? null);

  /** Charge is capped at 100 for the bar; overcharging shows in the text. */
  const charge = $derived(battery?.percentage ?? null);

  const tone = $derived.by(() => {
    if (charge === null) return 'auto' as const;
    if (battery?.is_charging) return 'accent' as const;
    if (charge <= 10) return 'danger' as const;
    if (charge <= 20) return 'warn' as const;
    return 'auto' as const;
  });

  /**
   * An estimate of time left, or `null` when there is nothing honest to show.
   *
   * Only shown while discharging. A machine on mains power has no time left to
   * report, and inventing one — as a naive derivation from charge and current
   * draw does, because it ignores the pack's actual capacity — would put a
   * confident 24-hour figure under a laptop that is plugged in.
   *
   * The fallback derivation uses the *design* capacity, so it is a
   * capacity-of-record estimate and says so.
   */
  const remaining = $derived.by(() => {
    const b = battery;
    if (!b || !b.present) return null;
    if (b.is_charging) {
      return b.time_to_full_minutes !== null
        ? { label: 'Charging in', minutes: b.time_to_full_minutes, estimated: false }
        : null;
    }
    if (b.time_to_empty_minutes !== null) {
      return { label: 'Remaining', minutes: b.time_to_empty_minutes, estimated: false };
    }
    if (b.ac_connected) return null;
    const { power_watts: draw, design_capacity_mah: mah, voltage_mv: mv } = b;
    if (!draw || draw < 0.1 || !mah || !mv) return null;
    // mAh × mV is microwatt-hours; the two factors of 1000 leave watt-hours.
    const wattHours = (mah * mv) / 1_000_000;
    if (charge === null || charge <= 0) return null;
    return {
      label: 'Remaining (est.)',
      minutes: ((charge / 100) * wattHours) / draw * 60,
      estimated: true,
    };
  });

  const healthTone = $derived.by(() => {
    const health = battery?.health_percent ?? null;
    if (health === null) return 'muted' as const;
    if (health < 70) return 'danger' as const;
    if (health < 85) return 'warn' as const;
    return 'default' as const;
  });
</script>

{#if !battery}
  <Empty reason="Waiting for the first sample…" />
{:else if !battery.present}
  <Empty reason="No battery detected" hint="This looks like a desktop or a Mac Studio." />
{:else}
  <div class="primary">
    <span class="value">{percent(charge)}</span>
    <span class="state" class:charging={battery.is_charging}>
      {#if battery.is_charging}
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6.6 1.2 2.4 7h2.8l-.8 3.8L9.6 5H6.8z" /></svg>
        Charging
      {:else if battery.ac_connected}
        Plugged in
      {:else}
        On battery
      {/if}
    </span>
  </div>

  <Bar value={charge} max={100} label="Charge" detail={percent(charge)} tone={tone} thickness={8} />

  <div class="stats">
    {#if remaining}
      <StatRow
        label={remaining.label}
        value={minutesToClock(remaining.minutes)}
        note={remaining.estimated ? 'at the current draw' : null}
      />
    {/if}
    {#if battery.power_watts !== null}
      <StatRow label="Power draw" value={watts(battery.power_watts)} />
    {/if}
    {#if battery.voltage_mv !== null}
      <StatRow label="Voltage" value={millivolts(battery.voltage_mv)} tone="muted" />
    {/if}
    {#if battery.temperature_c !== null}
      <StatRow label="Temperature" value={celsius(battery.temperature_c)} tone="muted" />
    {/if}
    {#if showHealth && battery.health_percent !== null}
      <StatRow
        label="Health"
        value={percent(battery.health_percent)}
        tone={healthTone}
        note={battery.cycle_count !== null ? `${battery.cycle_count} cycles` : null}
      />
    {/if}
    {#if showHealth && battery.cycle_count !== null && battery.health_percent === null}
      <StatRow label="Cycles" value={battery.cycle_count} tone="muted" />
    {/if}
  </div>
{/if}

<style>
  .primary {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .value {
    font-size: 1.55rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 0.73rem;
    color: var(--text-dim);
  }

  .state.charging {
    color: var(--accent);
  }

  .state svg {
    width: 10px;
    height: 10px;
    fill: currentColor;
  }

  .stats {
    display: grid;
    gap: 4px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
</style>
