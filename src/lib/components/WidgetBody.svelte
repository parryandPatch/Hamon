<script lang="ts">
  /**
   * Maps a layout widget to its renderer.
   *
   * A component registry rather than a `{#if}`/`{:else if}` chain: the type of
   * `body` is a function of the kind, so the `{#if}` form loses the narrowing
   * that a dictionary of components keeps.
   */
  import type { Component } from 'svelte';
  import type { Config } from '../catalog';
  import Gauge from '../widgets/Gauge.svelte';
  import History from '../widgets/History.svelte';
  import CpuCores from '../widgets/CpuCores.svelte';
  import Memory from '../widgets/Memory.svelte';
  import Gpu from '../widgets/Gpu.svelte';
  import Network from '../widgets/Network.svelte';
  import NetworkInterfaces from '../widgets/NetworkInterfaces.svelte';
  import Disk from '../widgets/Disk.svelte';
  import Filesystems from '../widgets/Filesystems.svelte';
  import DiskIo from '../widgets/DiskIo.svelte';
  import Temperatures from '../widgets/Temperatures.svelte';
  import Processes from '../widgets/Processes.svelte';
  import Battery from '../widgets/Battery.svelte';
  import System from '../widgets/System.svelte';
  import Uptime from '../widgets/Uptime.svelte';
  import CustomText from '../widgets/CustomText.svelte';
  import CustomAscii from '../widgets/CustomAscii.svelte';

  /**
   * Every widget is called with `{ config }`, whether or not it has options.
   *
   * `any` rather than `unknown` for one documented reason: Svelte types a
   * component that declares no props as `Component<Record<string, never>>`, and
   * there is no single parameter type that is a supertype of both that and
   * `Component<{ config: Config }>` — so a fully typed union would need the
   * empty-props widgets to declare a prop they do not use. This registry is the
   * only place the mismatch is bridged; every call site still passes exactly
   * `{ config }`.
   */
  type Body = Component<any>;

  const BODIES: Record<string, Body> = {
    gauge: Gauge,
    history: History,
    'cpu-cores': CpuCores,
    memory: Memory,
    gpu: Gpu,
    network: Network,
    'network-interfaces': NetworkInterfaces,
    disk: Disk,
    'disk-filesystems': Filesystems,
    'disk-io': DiskIo,
    temperatures: Temperatures,
    processes: Processes,
    battery: Battery,
    system: System,
    uptime: Uptime,
    'custom-text': CustomText,
    'custom-ascii': CustomAscii,
  };

  interface Props {
    kind: string;
    config: Config;
  }

  const { kind, config }: Props = $props();

  const Body = $derived(BODIES[kind]);
</script>

{#if Body}
  <Body {config} />
{:else}
  <!-- A layout saved by a newer build, or hand-edited. Say so rather than
       rendering an empty card that looks broken. -->
  <p class="unknown">Unknown widget kind <code>{kind}</code>. Remove it, or reset the layout to defaults.</p>
{/if}

<style>
  .unknown {
    margin: 0;
    font-size: 0.76rem;
    color: var(--warn);
    line-height: 1.4;
  }

  code {
    font-family: var(--mono);
    font-size: 0.95em;
    background: var(--hover);
    padding: 1px 4px;
    border-radius: 4px;
  }
</style>
