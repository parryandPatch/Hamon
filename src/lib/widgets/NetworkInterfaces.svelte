<script lang="ts">
  /**
   * The `network-interfaces` widget: a row per interface.
   *
   * Every interface is listed, including the ones carrying no traffic, because
   * the useful question here is often "why is my traffic going over `utun3` and
   * not `en0`" — which needs the full picture to answer.
   *
   * Interfaces with no address and no traffic are dimmed rather than hidden:
   * they are usually inactive VPNs and virtual bridges, and hiding them would
   * hide the very thing the user was looking for.
   */
  import StatRow from '../components/StatRow.svelte';
  import Empty from '../components/Empty.svelte';
  import { dashboard } from '../state.svelte';
  import { rate } from '../format';
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const net = $derived(dashboard.snapshot?.network ?? null);

  const interfaces = $derived.by(() => {
    const all = net?.interfaces ?? [];
    const wanted = String(config.interface ?? 'auto');
    return wanted === 'auto' ? all : all.filter((i) => i.name === wanted);
  });

  /** Nothing to show when even the loopback is missing. */
  const hasAny = $derived(interfaces.length > 0);
</script>

{#if !net}
  <Empty reason="Waiting for the first sample…" />
{:else if !hasAny}
  <Empty reason="No network interfaces reported" />
{:else}
  <ul class="list">
    {#each interfaces as iface (iface.name)}
      {@const idle = iface.rx_bytes_per_sec === 0 && iface.tx_bytes_per_sec === 0}
      <li class:idle class:down={!iface.is_up}>
        <div class="head">
          <span class="name">
            {#if !iface.is_up}<span class="dot" title="Interface is down" aria-hidden="true"></span>{/if}
            {iface.name}
          </span>
          <span class="rates" class:idle-rates={idle}>
            <span class="down">↓ {rate(iface.rx_bytes_per_sec)}</span>
            <span class="up">↑ {rate(iface.tx_bytes_per_sec)}</span>
          </span>
        </div>
        {#if iface.ip}
          <div class="detail">{iface.ip}</div>
        {/if}
        {#if iface.rx_errors > 0 || iface.tx_errors > 0}
          <div class="errors">
            {iface.rx_errors} rx / {iface.tx_errors} tx errors
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  <StatRow
    label="Session total"
    value="{rate(net.rx_bytes_per_sec)} down / {rate(net.tx_bytes_per_sec)} up"
    tone="muted"
  />
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 7px;
    /* A machine can have twenty interfaces; the list scrolls instead of
       stretching the card to fit them all. */
    overflow-y: auto;
    min-height: 0;
  }

  li {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .name {
    font-size: 0.82rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
  }

  .rates {
    display: flex;
    gap: 8px;
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .rates.idle-rates {
    color: var(--muted);
  }

  .down {
    color: var(--accent);
  }

  .up {
    color: var(--up);
  }

  .detail {
    font-size: 0.7rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .errors {
    font-size: 0.7rem;
    color: var(--warn);
  }

  li.idle .name {
    color: var(--text-dim);
  }

  li.down .name {
    color: var(--muted);
  }
</style>
