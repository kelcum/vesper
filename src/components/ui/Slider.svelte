<script lang="ts">
  interface Props {
    value: number;
    min: number;
    max: number;
    step?: number;
    label: string;
    unit?: string;
    onchange?: (value: number) => void;
  }
  let { value = $bindable(), min, max, step = 1, label, unit = '', onchange }: Props = $props();
  const pct = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="slider">
  <input
    type="range"
    aria-label={label}
    {min}
    {max}
    {step}
    bind:value
    style:--pct="{pct}%"
    onchange={() => onchange?.(value)}
  />
  <output>{value}{unit}</output>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 220px;
  }
  input {
    flex: 1;
    appearance: none;
    height: 22px;
    background: transparent;
    cursor: pointer;
  }
  input::-webkit-slider-runnable-track {
    height: 5px;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--accent) var(--pct), var(--surface-3) var(--pct));
  }
  input::-webkit-slider-thumb {
    appearance: none;
    width: 17px;
    height: 17px;
    margin-top: -6px;
    border-radius: 50%;
    background: #fff;
    box-shadow:
      0 1px 4px rgba(0, 0, 0, 0.4),
      0 0 0 4px color-mix(in srgb, var(--accent) 25%, transparent);
    transition: transform var(--dur-fast) var(--ease-out);
  }
  input:active::-webkit-slider-thumb {
    transform: scale(1.12);
  }
  output {
    min-width: 46px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-2);
  }
</style>
