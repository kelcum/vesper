<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, scale } from 'svelte/transition';

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    width?: number;
    onclose?: () => void;
    children?: Snippet;
    actions?: Snippet;
  }
  let { open = $bindable(), title, description, width = 460, onclose, children, actions }: Props = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let lastFocus: Element | null = null;

  function close() {
    open = false;
    onclose?.();
  }

  $effect(() => {
    if (open) {
      lastFocus = document.activeElement;
      queueMicrotask(() => {
        const first = panel?.querySelector<HTMLElement>(
          'button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        first?.focus();
      });
    } else if (lastFocus instanceof HTMLElement) {
      lastFocus.focus();
    }
  });

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
    if (e.key === 'Tab' && panel) {
      const items = [...panel.querySelectorAll<HTMLElement>('button:not([disabled]), input, select, textarea')];
      if (!items.length) return;
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div class="scrim" transition:fade={{ duration: 140 }} onclick={close} aria-hidden="true"></div>
  <div class="wrap">
    <div
      bind:this={panel}
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="dialog-title"
      style:max-width="{width}px"
      transition:scale={{ duration: 180, start: 0.96 }}
    >
      <h2 id="dialog-title">{title}</h2>
      {#if description}<p class="desc">{description}</p>{/if}
      {#if children}<div class="body">{@render children()}</div>{/if}
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(5, 4, 12, 0.55);
    backdrop-filter: blur(3px);
    z-index: 50;
  }
  .wrap {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    padding: var(--space-6);
    z-index: 51;
    pointer-events: none;
  }
  .panel {
    pointer-events: auto;
    width: 100%;
    max-height: calc(100vh - 80px);
    overflow: auto;
    padding: var(--space-6);
    border-radius: var(--radius-xl);
    background: var(--surface-solid);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-lg);
  }
  h2 {
    font-size: var(--text-xl);
  }
  .desc {
    margin-top: var(--space-2);
    color: var(--text-2);
  }
  .body {
    margin-top: var(--space-4);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-6);
  }
</style>
