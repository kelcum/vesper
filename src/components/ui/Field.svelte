<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    hint?: string;
    warning?: string;
    stacked?: boolean;
    children: Snippet;
  }
  let { label, hint, warning, stacked = false, children }: Props = $props();
</script>

<div class="field" class:stacked>
  <div class="copy">
    <div class="label">{label}</div>
    {#if hint}<div class="hint">{hint}</div>{/if}
    {#if warning}<div class="warning">{warning}</div>{/if}
  </div>
  <div class="control">{@render children()}</div>
</div>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-4) 0;
    border-top: 1px solid var(--border);
  }
  .field:first-child {
    border-top: 0;
  }
  .stacked {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-3);
  }
  .copy {
    flex: 1;
    min-width: 0;
  }
  .label {
    font-weight: 550;
  }
  .hint {
    margin-top: 2px;
    color: var(--text-2);
    font-size: var(--text-sm);
    max-width: 62ch;
  }
  .warning {
    margin-top: 6px;
    color: var(--warn);
    font-size: var(--text-sm);
    max-width: 62ch;
  }
  .control {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .stacked .control {
    flex-wrap: wrap;
  }
</style>
