<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    tone?: 'ok' | 'warn' | 'bad' | 'info' | 'neutral' | 'accent';
    title?: string;
    dot?: boolean;
    children: Snippet;
  }
  let { tone = 'neutral', title, dot = false, children }: Props = $props();
</script>

<span class="badge {tone}" {title}>
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
  <span class="text">{@render children()}</span>
</span>

<style>
  .badge {
    --c: var(--text-2);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    max-width: 100%;
    padding: 0 8px;
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    font-weight: 600;
    white-space: nowrap;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 13%, transparent);
    border: 1px solid color-mix(in srgb, var(--c) 22%, transparent);
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ok {
    --c: var(--ok);
  }
  .warn {
    --c: var(--warn);
  }
  .bad {
    --c: var(--bad);
  }
  .info {
    --c: var(--info);
  }
  .accent {
    --c: var(--accent-strong);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 22%, transparent);
  }
</style>
