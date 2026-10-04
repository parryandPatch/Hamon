<script lang="ts">
  /**
   * The `custom-ascii` widget.
   *
   * Fixed-width text in a `<pre>`. The only rules that matter:
   *
   * - `white-space: pre` so leading spaces survive, which they must or the art
   *   collapses.
   * - `overflow-x: auto`, because art is pasted at whatever width it was drawn
   *   at and it is better to scroll than to be silently mangled.
   * - Tab size pinned, since a literal tab in pasted art would otherwise render
   *   at the browser default and shift every row after it.
   */
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const art = $derived.by(() => {
    const raw = config.art;
    if (typeof raw !== 'string') return '';
    // A stray CR from a Windows-copied snippet would show as an empty column.
    return raw.replace(/\r\n?/g, '\n').replace(/\n+$/, '');
  });

  const rows = $derived(art === '' ? 0 : art.split('\n').length);
</script>

{#if art === ''}
  <p class="empty">No artwork set — open settings and paste some ASCII.</p>
{:else}
  <div class="art" class:wide={art.split('\n').some((l) => l.length > 80)}>
    <pre>{art}</pre>
  </div>
  <p class="foot">{rows} {rows === 1 ? 'row' : 'rows'}</p>
{/if}

<style>
  .art {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  pre {
    margin: 0;
    font-family: var(--mono);
    /* Small enough to fit a standard 40-column banner in a half-width card,
       large enough to stay legible. */
    font-size: clamp(0.5rem, 1.5cqw, 0.72rem);
    line-height: 1.15;
    tab-size: 4;
    white-space: pre;
    color: var(--text);
    /* Text art is blocky; antialiasing is what makes it look wrong. */
    -webkit-font-smoothing: antialiased;
  }

  .art.wide {
    /* Art wider than a comfortable card is scaled down to fit its height rather
       than scrolled, which keeps the whole piece visible. */
    overflow: hidden;
  }

  .art.wide pre {
    transform-origin: top left;
  }

  .foot {
    margin: 0;
    padding-top: 6px;
    border-top: 1px solid var(--border);
    font-size: 0.66rem;
    color: var(--muted);
  }

  .empty {
    margin: 0;
    font-size: 0.78rem;
    color: var(--muted);
  }
</style>
