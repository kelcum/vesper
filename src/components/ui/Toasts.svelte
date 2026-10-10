<script lang="ts">
  import { CircleAlert, CircleCheck, Info, TriangleAlert } from '@lucide/svelte';
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import { app } from '../../lib/store.svelte';

  const icons = { ok: CircleCheck, info: Info, warn: TriangleAlert, bad: CircleAlert };
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each app.toasts as t (t.id)}
    {@const Icon = icons[t.tone]}
    <div
      class="toast {t.tone}"
      animate:flip={{ duration: 200 }}
      in:fly={{ y: 12, duration: 220 }}
      out:fly={{ x: 24, duration: 160 }}
    >
      <Icon size={16} strokeWidth={2} aria-hidden="true" />
      <span>{t.message}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--space-5);
    bottom: var(--space-5);
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: min(420px, calc(100vw - 40px));
  }
  .toast {
    --c: var(--info);
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 11px 14px;
    border-radius: var(--radius-md);
    background: var(--surface-solid);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-md);
    font-size: var(--text-sm);
    line-height: 1.45;
  }
  .toast :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--c);
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
</style>
