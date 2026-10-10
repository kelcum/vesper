<script lang="ts" generics="T extends string">
  interface Option {
    value: T;
    label: string;
    title?: string;
  }
  interface Props {
    options: Option[];
    value: T;
    label: string;
    size?: 'sm' | 'md';
    onchange?: (value: T) => void;
  }
  let { options, value = $bindable(), label, size = 'md', onchange }: Props = $props();

  function pick(v: T) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }

  function onkeydown(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      pick(options[(i + 1) % options.length].value);
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      pick(options[(i - 1 + options.length) % options.length].value);
    }
  }
</script>

<div class="seg {size}" role="radiogroup" aria-label={label} tabindex="-1" {onkeydown}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      title={o.title}
      class:active={o.value === value}
      onclick={() => pick(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: var(--radius-md);
    background: var(--surface);
    border: 1px solid var(--border);
  }
  button {
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--text-2);
    font-weight: 550;
    font-size: var(--text-sm);
    cursor: pointer;
    transition:
      background var(--dur) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .sm button {
    height: 24px;
    padding: 0 9px;
    font-size: var(--text-xs);
  }
  button:hover {
    color: var(--text);
  }
  button.active {
    background: var(--surface-3);
    color: var(--text);
    box-shadow:
      inset 0 1px 0 var(--highlight),
      var(--shadow-sm);
  }
</style>
