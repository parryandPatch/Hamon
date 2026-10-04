<script lang="ts">
  /**
   * The empty state every widget falls back to.
   *
   * Worth having as a component rather than an `{#if}` per widget: a metric
   * being unavailable is the *normal* case on a machine without the hardware
   * for it, so the difference between "no GPU" and "the GPU widget is broken"
   * is mostly whether the message says something specific.
   */
  interface Props {
    reason: string;
    hint?: string;
  }

  const { reason, hint }: Props = $props();
</script>

<div class="empty">
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <path
      d="M12 3v2m0 14v2M3 12h2m14 0h2M5.6 5.6l1.4 1.4m10 10 1.4 1.4m0-12.8-1.4 1.4m-10 10L5.6 18.4"
      stroke="currentColor"
      stroke-width="1.6"
      stroke-linecap="round"
      fill="none"
    />
    <circle cx="12" cy="12" r="3.2" stroke="currentColor" stroke-width="1.6" fill="none" />
  </svg>
  <p class="reason">{reason}</p>
  {#if hint}<p class="hint">{hint}</p>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    text-align: center;
    padding: 14px 8px;
    min-height: 72px;
    color: var(--muted);
  }

  svg {
    width: 22px;
    height: 22px;
    opacity: 0.75;
  }

  .reason {
    margin: 0;
    font-size: 0.79rem;
    color: var(--text-dim);
  }

  .hint {
    margin: 0;
    font-size: 0.72rem;
    max-width: 34ch;
  }
</style>