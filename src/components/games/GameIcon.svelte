<script lang="ts">
  import { Gamepad2 } from '@lucide/svelte';

  interface Props {
    url: string | null | undefined;
    size?: number;
    alt?: string;
  }
  let { url, size = 56, alt = '' }: Props = $props();
  let failed = $state(false);
  $effect(() => {
    void url;
    failed = false;
  });
</script>

<span class="icon" style:width="{size}px" style:height="{size}px">
  {#if url && !failed}
    <img src={url} {alt} width={size} height={size} loading="lazy" onerror={() => (failed = true)} />
  {:else}
    <Gamepad2 size={Math.round(size * 0.42)} strokeWidth={1.6} aria-hidden="true" />
  {/if}
</span>

<style>
  .icon {
    display: grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: 12px;
    color: var(--text-3);
    background: var(--surface-3);
    border: 1px solid var(--border);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
