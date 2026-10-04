<script lang="ts">
  /**
   * The per-widget configuration panel.
   *
   * Generated from the catalog's field descriptions rather than hand-written per
   * widget, so adding a configurable option is a one-line change in
   * `catalog.ts` and nothing here has to know about it.
   *
   * Edits are applied to a local draft and committed on change (not on a Save
   * button), because a hardware dashboard is something you tweak while
   * watching it — a Save button would make you lose sight of the live numbers
   * every time you adjusted something.
   */
  import type { CatalogEntry, Config, ConfigValue, Field } from '../catalog';
  import { dashboard } from '../state.svelte';

  interface Props {
    index: number;
    entry: CatalogEntry;
    config: Config;
  }

  const { index, entry, config }: Props = $props();

  /**
   * The value being edited, seeded from the stored config.
   *
   * Seeded once, and deliberately never re-synced from `config`. The panel is
   * mounted fresh each time its widget's settings open, so the seed is already
   * the current value, and any effect comparing the two afterwards is a feedback
   * loop: the `lines` field holds an array, and two arrays holding the same text
   * are never `===`, so the comparison would write on every run and Svelte would
   * abort the effect with `effect_update_depth_exceeded`.
   *
   * Values the backend would clamp need no reconciliation either —
   * `resolveConfig` discards anything out of range before it reaches here, so
   * what was stored is what was accepted.
   */
  // svelte-ignore state_referenced_locally
  let draft = $state<Config>({ ...config });

  function commit(): void {
    // Only send a field the widget actually declares, so a payload can never
    // gain a key the backend has no default for.
    const payload: Config = {};
    for (const field of entry.fields) payload[field.key] = draft[field.key] ?? entry.defaults[field.key] ?? null;
    dashboard.configureWidget(index, payload);
  }

  function onSelect(key: string, event: Event): void {
    draft[key] = (event.currentTarget as HTMLSelectElement).value;
    commit();
  }

  function onNumber(key: string, event: Event): void {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    // An empty or half-typed field is not a value yet; ignore it rather than
    // committing NaN and having the widget clamp it away.
    if (!Number.isFinite(value)) return;
    draft[key] = value;
    commit();
  }

  function onBoolean(key: string, event: Event): void {
    draft[key] = (event.currentTarget as HTMLInputElement).checked;
    commit();
  }

  function onLines(event: Event): void {
    draft.lines = (event.currentTarget as HTMLTextAreaElement).value.split('\n');
    commit();
  }

  function onAscii(event: Event): void {
    draft.art = (event.currentTarget as HTMLTextAreaElement).value;
    commit();
  }

  /**
   * The choices for a `select` field.
   *
   * Some fields are fixed (`sort`, `style`) and come straight from the catalog;
   * others name a *device* — a GPU, an interface, a mount point — so their
   * choices are whatever the last snapshot reported. Live choices win when there
   * are any, because an option naming an interface that has since disappeared
   * would be worse than useless.
   */
  function optionsFor(field: Field): { value: string; label: string }[] {
    return liveOptions(field.key) ?? field.options ?? [];
  }

  /** Live choices for the device-naming fields, or `null` if none apply. */
  function liveOptions(key: string): { value: string; label: string }[] | null {
    const snapshot = dashboard.snapshot;
    if (!snapshot) return null;
    if (key === 'device') {
      return [
        { value: 'auto', label: 'All devices' },
        ...snapshot.gpu.devices.map((d) => ({ value: String(d.index), label: d.name })),
      ];
    }
    if (key === 'interface') {
      return [
        { value: 'auto', label: 'All interfaces' },
        ...snapshot.network.interfaces.map((i) => ({ value: i.name, label: i.name })),
      ];
    }
    if (key === 'mount') {
      return [
        { value: 'auto', label: 'All mount points' },
        ...snapshot.disk.filesystems.map((f) => ({ value: f.mount_point, label: f.mount_point })),
      ];
    }
    return null;
  }

  function current(field: { key: string }): ConfigValue {
    return draft[field.key];
  }
</script>

<div class="panel">
  {#each entry.fields as field (field.key)}
    <label class="field">
      <span class="label">{field.label}</span>

      {#if field.kind === 'select'}
        <select
          value={String(current(field) ?? '')}
          onchange={(e) => onSelect(field.key, e)}
        >
          {#each optionsFor(field) as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      {:else if field.kind === 'number'}
        <span class="number">
          <input
            type="number"
            value={Number(current(field) ?? 0)}
            min={field.min}
            max={field.max}
            step={field.step ?? 1}
            onchange={(e) => onNumber(field.key, e)}
          />
          {#if field.unit}<span class="unit">{field.unit}</span>{/if}
        </span>
      {:else if field.kind === 'boolean'}
        <input
          class="checkbox"
          type="checkbox"
          checked={Boolean(current(field))}
          onchange={(e) => onBoolean(field.key, e)}
        />
      {:else if field.kind === 'lines'}
        <textarea
          rows="3"
          spellcheck="false"
          value={Array.isArray(current(field)) ? (current(field) as string[]).join('\n') : ''}
          onchange={onLines}
        ></textarea>
      {:else if field.kind === 'ascii'}
        <textarea
          class="ascii"
          rows="6"
          spellcheck="false"
          value={String(current(field) ?? '')}
          onchange={onAscii}
        ></textarea>
      {:else}
        <input type="text" value={String(current(field) ?? '')} />
      {/if}
    </label>
  {:else}
    <p class="none">This widget has no options.</p>
  {/each}
</div>

<style>
  .panel {
    display: grid;
    gap: 9px;
    padding: 10px;
    border-top: 1px dashed var(--border);
    background: var(--panel-sunken);
  }

  .field {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .label {
    font-size: 0.74rem;
    color: var(--text-dim);
    white-space: nowrap;
  }

  select,
  input[type='text'],
  input[type='number'],
  textarea {
    width: 100%;
    min-width: 0;
    padding: 5px 7px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--input);
    color: var(--text);
    font: inherit;
    font-size: 0.78rem;
  }

  textarea {
    resize: vertical;
    font-family: var(--mono);
    font-size: 0.72rem;
    line-height: 1.4;
  }

  textarea.ascii {
    white-space: pre;
    tab-size: 4;
  }

  select:focus-visible,
  input:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .number {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .unit {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .checkbox {
    width: 15px;
    height: 15px;
    justify-self: start;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .none {
    margin: 0;
    font-size: 0.72rem;
    color: var(--muted);
  }

  /* The panel spans the full card width even inside a narrow grid column, so
     the controls do not get squeezed into a 3-character-wide select. */
  @media (min-width: 1px) {
    .panel {
      grid-column: 1 / -1;
    }
  }
</style>
