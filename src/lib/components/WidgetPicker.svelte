<script lang="ts">
  /**
   * The widget picker, shown while edit mode is on.
   *
   * Grouped because seventeen flat rows is a list nobody scans. Each row shows
   * what the widget is for, since "Disk" alone does not tell you whether you
   * want throughput or filesystem capacity.
   *
   * Adding a widget does not close the panel: the dashboard has a picking
   * session in it, and making the user re-open edit mode for each of three
   * widgets would be the wrong default.
   */
  import { CATALOG, type CatalogEntry } from '../catalog';
  import { dashboard } from '../state.svelte';

  const groups = $derived.by(() => {
    const order: CatalogEntry['group'][] = ['Metrics', 'Hardware', 'Storage & Network', 'System', 'Custom'];
    const byGroup = new Map<string, typeof CATALOG>();
    for (const entry of CATALOG) {
      const bucket = byGroup.get(entry.group) ?? [];
      bucket.push(entry);
      byGroup.set(entry.group, bucket);
    }
    // Only groups that actually have entries survive, so an emptied group does
    // not leave a stray heading.
    return order
      .filter((name) => (byGroup.get(name)?.length ?? 0) > 0)
      .map((name) => ({ name, entries: byGroup.get(name) ?? [] }));
  });

  /** How many of each kind the dashboard already has, for the count badge. */
  function countOf(kind: string): number {
    return dashboard.layout.widgets.filter((w) => w.kind === kind).length;
  }
</script>

<div class="picker">
  <div class="picker-head">
    <h3>Add a widget</h3>
    <span class="hint">Click to add · use the handles on each card to reorder, configure or remove</span>
  </div>

  <div class="groups">
    {#each groups as group (group.name)}
      <section>
        <h4>{group.name}</h4>
        <ul>
          {#each group.entries as entry (entry.kind)}
            {@const have = countOf(entry.kind)}
            <li>
              <button type="button" onclick={() => dashboard.addWidget(entry.kind)}>
                <span class="text">
                  <span class="label">{entry.label}</span>
                  <span class="summary">{entry.summary}</span>
                </span>
                {#if have > 0}
                  <span class="count" title="{have} already on the dashboard">×{have}</span>
                {/if}
                <svg class="plus" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M8 3.4v9.2M3.4 8h9.2" />
                </svg>
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
</div>

<style>
  .picker {
    display: grid;
    gap: 12px;
    padding: 14px 16px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--panel);
    box-shadow: var(--shadow);
  }

  .picker-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .hint {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 14px 20px;
  }

  section {
    min-width: 0;
  }

  h4 {
    margin: 0 0 5px;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }

  button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 110ms, border-color 110ms;
  }

  button:hover {
    background: var(--hover);
    border-color: var(--border);
  }

  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .text {
    display: grid;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .label {
    font-size: 0.8rem;
    font-weight: 500;
  }

  .summary {
    font-size: 0.68rem;
    color: var(--muted);
    line-height: 1.3;
  }

  .count {
    font-size: 0.66rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    background: var(--hover);
    padding: 1px 5px;
    border-radius: 999px;
    flex: none;
  }

  .plus {
    width: 13px;
    height: 13px;
    flex: none;
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.6;
    stroke-linecap: round;
    opacity: 0;
    transition: opacity 110ms;
  }

  button:hover .plus {
    opacity: 1;
  }
</style>
