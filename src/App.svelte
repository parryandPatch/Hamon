<script lang="ts">
  /**
   * The dashboard shell: header, widget grid, and edit mode.
   *
   * All state lives in `state.svelte.ts`; this file is the layout and the event
   * wiring. Drag-to-reorder is handled here with delegation on the grid rather
   * than per-card handlers, because `dragleave`/`dragenter` pairs per card
   * produce spurious drops at the gaps between cards, and one delegated handler
   * asking "which card is under the pointer" does not.
   */
  import { onDestroy } from 'svelte';
  import { dashboard } from './lib/state.svelte';
  import { entryFor, resolveConfig, describeConfig } from './lib/catalog';
  import WidgetFrame from './lib/components/WidgetFrame.svelte';
  import WidgetBody from './lib/components/WidgetBody.svelte';
  import WidgetConfigEditor from './lib/components/WidgetConfigEditor.svelte';
  import WidgetPicker from './lib/components/WidgetPicker.svelte';
  import Empty from './lib/components/Empty.svelte';
  import { duration } from './lib/format';

  onDestroy(() => dashboard.stop());
  void dashboard.start();

  /** Card the dragged card would land on, or `null`. */
  let dropTarget = $state<number | null>(null);

  const widgets = $derived(dashboard.layout.widgets);
  const total = $derived(widgets.length);

  /** Tick age, so a stalled sampler is visible instead of silently frozen. */
  const age = $derived.by(() => {
    const snapshot = dashboard.snapshot;
    if (!snapshot) return null;
    return Math.max(0, Date.now() - snapshot.timestamp_ms);
  });

  function cardIndex(target: EventTarget | null): number | null {
    if (!(target instanceof Element)) return null;
    const card = target.closest('[data-index]');
    if (!card) return null;
    const index = Number(card.getAttribute('data-index'));
    return Number.isInteger(index) ? index : null;
  }

  function onDragStart(event: DragEvent): void {
    if (!dashboard.editing) return;
    const index = cardIndex(event.target);
    if (index === null) return;
    dashboard.dragging = index;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      // Some webviews only start a drag when data is set; the payload itself is
      // unused because the drop handler reads `dashboard.dragging`.
      event.dataTransfer.setData('text/plain', String(index));
    }
  }

  function onDragOver(event: DragEvent): void {
    if (!dashboard.editing || dashboard.dragging === null) return;
    // Required, or the browser refuses the drop.
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    dropTarget = cardIndex(event.target);
  }

  function onDrop(event: DragEvent): void {
    if (!dashboard.editing || dashboard.dragging === null) return;
    event.preventDefault();
    const from = dashboard.dragging;
    const to = cardIndex(event.target);
    dashboard.dragging = null;
    dropTarget = null;
    // Dropping a card on itself is a no-op, not an error.
    if (to !== null && to !== from) dashboard.moveWidget(from, to);
  }

  function onDragEnd(): void {
    dashboard.dragging = null;
    dropTarget = null;
  }

  function toggleEdit(): void {
    dashboard.editing = !dashboard.editing;
    if (!dashboard.editing) dashboard.configuring = null;
  }
</script>

<div class="app" class:editing={dashboard.editing}>
  <header class="topbar">
    <div class="identity">
      <span class="name">Hamon</span>
      {#if dashboard.snapshot}
        <span class="host">{dashboard.snapshot.system.hostname}</span>
      {/if}
    </div>

    <div class="controls">
      {#if dashboard.editing}
        <label class="control">
          <span>Columns</span>
          <select
            value={String(dashboard.layout.columns)}
            onchange={(e) => dashboard.setColumns(Number((e.currentTarget as HTMLSelectElement).value))}
          >
            {#each [1, 2, 3, 4] as n (n)}
              <option value={String(n)}>{n}</option>
            {/each}
          </select>
        </label>

        <label class="control">
          <span>Refresh</span>
          <select
            value={String(dashboard.intervalMs)}
            onchange={(e) => dashboard.setInterval(Number((e.currentTarget as HTMLSelectElement).value))}
          >
            {#each [250, 500, 1000, 2000, 5000] as ms (ms)}
              <option value={String(ms)}>{ms < 1000 ? `${ms}ms` : `${ms / 1000}s`}</option>
            {/each}
          </select>
        </label>

        <button type="button" class="ghost" onclick={() => dashboard.resetLayout()}>Reset</button>
        <button type="button" class="primary" onclick={toggleEdit}>Done</button>
      {:else}
        <span class="status" class:paused={!dashboard.running}>
          {#if !dashboard.running}
            Paused
          {:else if age === null}
            Connecting…
          {:else if age > 3000}
            Stalled ({duration(age / 1000)} old)
          {:else}
            Live
          {/if}
        </span>

        <button
          type="button"
          class="icon"
          title={dashboard.running ? 'Pause sampling' : 'Resume sampling'}
          aria-label={dashboard.running ? 'Pause sampling' : 'Resume sampling'}
          onclick={() => dashboard.toggleRunning()}
        >
          {#if dashboard.running}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5.5 3.5v9M10.5 3.5v9" /></svg>
          {:else}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3.2 12.4 8 5 12.8z" /></svg>
          {/if}
        </button>

        <button type="button" class="primary" onclick={toggleEdit}>Edit dashboard</button>
      {/if}
    </div>
  </header>

  {#if dashboard.notice}
    <div class="notice" role="status">
      {dashboard.notice}
      <button type="button" class="dismiss" aria-label="Dismiss" onclick={() => dashboard.setNotice(null)}>×</button>
    </div>
  {/if}

  {#if !dashboard.ready}
    <div class="splash">
      <span class="spinner" aria-hidden="true"></span>
      <p>Starting Hamon…</p>
    </div>
  {:else if dashboard.editing}
    <WidgetPicker />
  {/if}

  {#if total === 0 && dashboard.ready}
    <div class="blank">
      {#if dashboard.editing}
        <!-- The picker is already on screen above, so pointing at it beats
             telling the user to switch modes they are already in. -->
        <Empty reason="Nothing on the dashboard yet" hint="Pick a widget from the list above." />
      {:else}
        <Empty reason="The dashboard is empty" hint="Turn on edit mode to add widgets." />
        <button type="button" class="primary" onclick={toggleEdit}>Edit dashboard</button>
      {/if}
    </div>
  {:else}
    <main
      class="grid"
      style:--columns={dashboard.layout.columns}
      ondragstart={onDragStart}
      ondragover={onDragOver}
      ondragleave={() => (dropTarget = null)}
      ondrop={onDrop}
      ondragend={onDragEnd}
    >
      {#each widgets as widget, index (index)}
        {@const entry = entryFor(widget)}
        {@const config = resolveConfig(widget)}
        <div class="cell" class:open={dashboard.configuring === index}>
          <WidgetFrame
            title={entry.label}
            status={dashboard.configuring === index ? null : describeConfig(entry, config)}
            {index}
            {total}
            spanAll={dashboard.configuring === index}
            {dropTarget}
          >
            <WidgetBody kind={widget.kind} {config} />
          </WidgetFrame>

          {#if dashboard.configuring === index}
            <WidgetConfigEditor {index} {entry} {config} />
          {/if}
        </div>
      {/each}
    </main>
  {/if}
</div>

<!--
  A one-line summary of the non-default configuration, shown in a card's header
  so a dashboard of eight similarly-named widgets is still self-describing.
-->

<style>
  .app {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100vh;
    /* The window is not a document; nothing here should rubber-band. */
    overflow: hidden;
    padding: 0 16px 16px;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    /* Bleeds into the title bar's drag region via the global rule in app.css;
       the padding keeps the text off the traffic lights. */
    padding: 14px 2px 4px;
  }

  .identity {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .name {
    font-size: 0.92rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .host {
    font-size: 0.76rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .control {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 0.7rem;
    color: var(--text-dim);
    white-space: nowrap;
  }

  select {
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--input);
    color: var(--text);
    font: inherit;
    font-size: 0.74rem;
  }

  .status {
    font-size: 0.72rem;
    color: var(--muted);
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }

  .status::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  .status.paused::before {
    background: var(--warn);
  }

  button {
    font: inherit;
    cursor: pointer;
    border-radius: 7px;
    transition: background 120ms, border-color 120ms, color 120ms;
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .primary {
    padding: 5px 11px;
    border: 1px solid var(--accent-dim);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.76rem;
    font-weight: 500;
  }

  .primary:hover {
    background: var(--accent-dim);
  }

  .ghost {
    padding: 5px 10px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-dim);
    font-size: 0.76rem;
  }

  .ghost:hover {
    background: var(--hover);
    color: var(--text);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-dim);
  }

  .icon:hover {
    background: var(--hover);
    color: var(--text);
  }

  .icon svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--danger-soft);
    color: var(--danger);
    font-size: 0.78rem;
    line-height: 1.35;
  }

  .dismiss {
    margin-left: auto;
    padding: 0 6px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 1.05rem;
    line-height: 1;
  }

  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(var(--columns, 2), minmax(0, 1fr));
    gap: 12px;
    align-content: start;
    /* Cards vary in height; letting each row take its tallest member stops a
       short gauge from being stretched to match a process table. */
    grid-auto-rows: min-content;
    overflow-y: auto;
    padding-bottom: 4px;
  }

  .cell {
    display: flex;
    flex-direction: column;
    min-width: 0;
    /* A grid item's default `stretch` would make a cell as tall as the tallest
       in its row, and the card would stretch with it. */
    align-self: start;
  }

  .cell :global(.card) {
    flex: 1;
  }

  /* Narrow windows: collapse columns rather than squeezing a process table into
     a 150 px track. */
  @media (max-width: 1040px) {
    .grid {
      grid-template-columns: repeat(min(var(--columns, 2), 2), minmax(0, 1fr));
    }
  }

  @media (max-width: 680px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .splash,
  .blank {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    flex: 1;
    min-height: 0;
  }

  .splash p {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-dim);
  }

  .spinner {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid var(--track);
    border-top-color: var(--accent);
    animation: spin 720ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Respect a reduced-motion preference: the numbers still update. */
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2.4s;
    }
  }
</style>
