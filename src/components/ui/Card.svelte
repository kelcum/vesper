<script lang="ts">
  import type { Component, Snippet } from 'svelte';

  interface Props {
    title?: string;
    subtitle?: string;
    icon?: Component<any>;
    padded?: boolean;
    actions?: Snippet;
    children: Snippet;
    id?: string;
  }
  let { title, subtitle, icon, padded = true, actions, children, id }: Props = $props();
</script>

<section class="card glass" class:padded {id}>
  {#if title || actions}
    <header>
      {#if icon}
        {@const Icon = icon}
        <span class="ico"><Icon size={16} strokeWidth={1.9} aria-hidden="true" /></span>
      {/if}
      <div class="titles">
        {#if title}<h3>{title}</h3>{/if}
        {#if subtitle}<p class="sub">{subtitle}</p>{/if}
      </div>
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    </header>
  {/if}
  {@render children()}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .padded {
    padding: var(--space-5);
  }
  header {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    margin-bottom: var(--space-3);
  }
  .ico {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 9px;
    color: var(--accent-strong);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  .sub {
    margin-top: 2px;
    color: var(--text-2);
    font-size: var(--text-sm);
  }
  .actions {
    display: flex;
    gap: var(--space-2);
    flex: none;
  }
</style>
