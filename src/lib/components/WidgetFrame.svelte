<script lang="ts">
  /**
   * The card chrome around every widget: title, optional status line, and — in
   * edit mode — the handles for configuring, moving and removing.
   *
   * Keeping the chrome in one place is what makes edit mode cheap: adding a
   * new widget type means writing its body, not re-implementing drag and drop
   * affordances on it.
   */
  import type { Snippet } from 'svelte';
  import { dashboard } from '../state.svelte';

  interface Props {
    title: string;
    /** Short right-aligned note in the header, e.g. a device name. */
    status?: string | null;
    /** Layout index, or `null` for a widget that is not in the layout. */
    index?: number | null;
    /** Total widget count, for the move buttons' bounds. */
    total?: number;
    /** Extra classes for the body area, e.g. a table that should not scroll. */
    bodyClass?: string;
    /**
     * Make the card span every grid column.
     *
     * Used for whichever card has its settings open: the panel below the widget
     * needs more width than one column of a four-column grid, and widening the
     * card in place keeps the panel visibly attached to what it configures
     * instead of sliding it into a drawer elsewhere.
     */
    spanAll?: boolean;
    /** Index this card would be dropped at, for the drop indicator. */
    dropTarget?: number | null;
    children: Snippet;
  }

  const {
    title,
    status = null,
    index = null,
    total = 0,
    bodyClass = '',
    spanAll = false,
    dropTarget = null,
    children,
  }: Props = $props();

  const isFirst = $derived(index === 0);
  const isLast = $derived(index !== null && index >= total - 1);
</script>

<section
  class="card"
  class:editing={dashboard.editing}
  class:dragging={dashboard.dragging === index}
  class:span-all={spanAll}
  class:drop-target={dropTarget === index}
  data-index={index ?? undefined}
  draggable={dashboard.editing && index !== null}
>
  <header>
    <h2 title={title}>{title}</h2>
    {#if status}
      <span class="status" title={status}>{status}</span>
    {/if}
    {#if dashboard.editing && index !== null}
      <div class="handles">
        <button
          type="button"
          class="handle"
          title="Move earlier"
          aria-label="Move {title} earlier"
          disabled={isFirst}
          onclick={() => dashboard.moveWidget(index, index - 1)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 12V4m0 0L4.5 7.5M8 4l3.5 3.5" /></svg>
        </button>
        <button
          type="button"
          class="handle"
          title="Move later"
          aria-label="Move {title} later"
          disabled={isLast}
          onclick={() => dashboard.moveWidget(index, index + 1)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 4v8m0 0 3.5-3.5M8 12l-3.5-3.5" /></svg>
        </button>
        <button
          type="button"
          class="handle"
          title="Settings"
          aria-label="Configure {title}"
          aria-pressed={dashboard.configuring === index}
          class:active={dashboard.configuring === index}
          onclick={() => (dashboard.configuring = dashboard.configuring === index ? null : index)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="8" r="2.1" />
            <path
              d="M8 1.8v1.7M8 12.5v1.7M14.2 8h-1.7M3.5 8H1.8m10.2-4.2-1.2 1.2M5.2 10.8 4 12m0-8 1.2 1.2m5.6 5.6L12 12"
            />
          </svg>
        </button>
        <button
          type="button"
          class="handle remove"
          title="Remove"
          aria-label="Remove {title}"
          onclick={() => dashboard.removeWidget(index)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
      </div>
    {/if}
  </header>

  <div class="body {bodyClass}">
    {@render children()}
  </div>
</section>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 12px 14px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel);
    box-shadow: var(--shadow);
    /* Sizing: widgets are rows in a grid, so they may shrink but never grow
       past the track, which is what keeps a table from widening the grid. */
    min-height: 0;
    overflow: hidden;
  }

  .card.span-all {
    grid-column: 1 / -1;
  }

  .card.drop-target {
    /* The left edge marks where the dragged card will land. Using the edge
       rather than the whole card avoids the target flickering as the pointer
       moves across it. */
    box-shadow: inset 3px 0 0 var(--accent), var(--shadow);
  }

  .card.dragging {
    opacity: 0.45;
  }

  .card.editing {
    border-color: var(--border-strong);
    /* A dashed edge is the clearest "this whole area is live" signal that does
       not compete with the widget's own content. */
    outline: 1px dashed var(--border-strong);
    outline-offset: -4px;
    border-radius: var(--radius);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    min-height: 22px;
  }

  h2 {
    margin: 0;
    font-size: 0.74rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status {
    margin-left: auto;
    font-size: 0.72rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 45%;
  }

  .handles {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }

  .status + .handles {
    margin-left: 0;
  }

  .handle {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    transition: background 120ms, color 120ms, border-color 120ms;
  }

  .handle svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .handle:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }

  .handle.active {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: var(--accent-dim);
  }

  .handle.remove:hover:not(:disabled) {
    color: var(--danger);
    background: var(--danger-soft);
  }

  .handle:disabled {
    opacity: 0.28;
    cursor: default;
  }

  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 9px;
    min-height: 0;
  }
</style>