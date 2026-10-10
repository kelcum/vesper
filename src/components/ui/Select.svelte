<script lang="ts" generics="T extends string">
  import { ChevronDown } from '@lucide/svelte';

  interface Props {
    value: T;
    options: { value: T; label: string }[];
    label: string;
    onchange?: (value: T) => void;
    disabled?: boolean;
  }
  let { value = $bindable(), options, label, onchange, disabled = false }: Props = $props();
</script>

<div class="select">
  <select
    aria-label={label}
    {disabled}
    bind:value
    onchange={() => onchange?.(value)}
  >
    {#each options as o (o.value)}
      <option value={o.value}>{o.label}</option>
    {/each}
  </select>
  <ChevronDown size={14} strokeWidth={2} aria-hidden="true" />
</div>

<style>
  .select {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  select {
    appearance: none;
    width: 100%;
    height: 34px;
    min-width: 160px;
    padding: 0 34px 0 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  select:hover {
    border-color: var(--border-strong);
  }
  select:focus-visible {
    box-shadow: var(--focus);
  }
  option {
    background: var(--surface-solid);
    color: var(--text);
  }
  .select :global(svg) {
    position: absolute;
    right: 11px;
    pointer-events: none;
    color: var(--text-3);
  }
</style>
