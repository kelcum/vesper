<script lang="ts">
  import type { Component, Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'lg';
    icon?: Component<any>;
    loading?: boolean;
    disabled?: boolean;
    type?: 'button' | 'submit';
    title?: string;
    label?: string;
    full?: boolean;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
  }

  let {
    variant = 'secondary',
    size = 'md',
    icon,
    loading = false,
    disabled = false,
    type = 'button',
    title,
    label,
    full = false,
    onclick,
    children,
  }: Props = $props();

  const iconSize = $derived(size === 'sm' ? 14 : size === 'lg' ? 18 : 16);
</script>

<button
  {type}
  {title}
  aria-label={label}
  class="btn {variant} {size}"
  class:full
  class:only={!children}
  disabled={disabled || loading}
  aria-busy={loading}
  {onclick}
>
  {#if loading}
    <span class="spin" aria-hidden="true"></span>
  {:else if icon}
    {@const Icon = icon}
    <Icon size={iconSize} strokeWidth={1.9} aria-hidden="true" />
  {/if}
  {#if children}<span class="text">{@render children()}</span>{/if}
</button>

<style>
  .btn {
    --h: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: var(--h);
    padding: 0 14px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-weight: 550;
    font-size: var(--text-md);
    white-space: nowrap;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out),
      box-shadow var(--dur) var(--ease-out);
    box-shadow: inset 0 1px 0 var(--highlight);
  }
  .btn:hover:not(:disabled) {
    background: var(--surface-3);
    border-color: var(--border-strong);
  }
  .btn:active:not(:disabled) {
    transform: translateY(1px) scale(0.985);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .sm {
    --h: 28px;
    padding: 0 10px;
    font-size: var(--text-sm);
    border-radius: var(--radius-sm);
  }
  .lg {
    --h: 46px;
    padding: 0 22px;
    font-size: var(--text-lg);
    border-radius: var(--radius-lg);
  }
  .only {
    width: var(--h);
    padding: 0;
  }
  .full {
    width: 100%;
  }
  .primary {
    background: var(--accent-gradient);
    border-color: transparent;
    color: #0b0a14;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.35),
      var(--accent-glow);
  }
  .primary:hover:not(:disabled) {
    background: var(--accent-gradient);
    filter: brightness(1.07);
    border-color: transparent;
  }
  .ghost {
    background: transparent;
    border-color: transparent;
    box-shadow: none;
    color: var(--text-2);
  }
  .ghost:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-2);
  }
  .danger {
    color: var(--bad);
  }
  .danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--bad) 14%, transparent);
    border-color: color-mix(in srgb, var(--bad) 35%, transparent);
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
