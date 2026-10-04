<script lang="ts">
  /**
   * The `custom-text` widget.
   *
   * Deliberately has no live data: it exists so a dashboard can carry a label,
   * a heading, a note, or a decorative line. It renders as plain text, left
   * aligned and proportional — `custom-ascii` is the one for artwork.
   */
  import type { Config } from '../catalog';

  interface Props {
    config: Config;
  }

  const { config }: Props = $props();

  const lines = $derived.by(() => {
    const raw = config.lines;
    if (Array.isArray(raw)) return raw.map(String);
    if (typeof raw === 'string') return raw.split('\n');
    return [];
  });
</script>

{#if lines.length === 0}
  <p class="empty">No lines set — open settings and add some.</p>
{:else}
  <div class="lines">
    {#each lines as line, i (i)}
      {#if line.trim() === ''}
        <span class="blank" aria-hidden="true">&nbsp;</span>
      {:else}
        <p>{line}</p>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .lines {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  p {
    margin: 0;
    font-size: 0.86rem;
    line-height: 1.35;
    /* Long pasted URLs must not widen the grid track. */
    overflow-wrap: anywhere;
  }

  .blank {
    display: block;
    height: 0.5em;
  }

  .empty {
    margin: 0;
    font-size: 0.78rem;
    color: var(--muted);
  }
</style>
