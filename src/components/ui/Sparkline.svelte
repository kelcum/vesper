<script lang="ts">
  interface Props {
    values: (number | null)[];
    max?: number;
    height?: number;
    color?: string;
    label: string;
  }
  let { values, max, height = 44, color = 'var(--accent-strong)', label }: Props = $props();

  const W = 240;
  const top = $derived(max ?? Math.max(1, ...values.filter((v): v is number => v != null)) * 1.15);
  const points = $derived.by(() => {
    const n = values.length;
    if (n < 2) return '';
    return values
      .map((v, i) => (v == null ? null : `${((i / (n - 1)) * W).toFixed(1)},${(height - (Math.min(v, top) / top) * (height - 2) - 1).toFixed(1)}`))
      .filter(Boolean)
      .join(' ');
  });
  const area = $derived(points ? `0,${height} ${points} ${W},${height}` : '');
  const id = `spark-${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" role="img" aria-label={label} style:height="{height}px">
  <defs>
    <linearGradient {id} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color={color} stop-opacity="0.28" />
      <stop offset="1" stop-color={color} stop-opacity="0" />
    </linearGradient>
  </defs>
  {#if points}
    <polygon points={area} fill="url(#{id})" />
    <polyline {points} fill="none" stroke={color} stroke-width="1.6" vector-effect="non-scaling-stroke" stroke-linejoin="round" />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    overflow: visible;
  }
</style>
