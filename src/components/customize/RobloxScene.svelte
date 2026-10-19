<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    height?: number;
    hideCursor?: boolean;
    onpointermove?: (e: PointerEvent) => void;
    onpointerleave?: (e: PointerEvent) => void;
    children: Snippet;
  }
  let { height = 200, hideCursor = false, onpointermove, onpointerleave, children }: Props = $props();
</script>

<div
  class="scene"
  class:no-cursor={hideCursor}
  style:height="{height}px"
  {onpointermove}
  {onpointerleave}
  role="presentation"
>
  <div class="sky" aria-hidden="true"></div>
  <div class="floor" aria-hidden="true"></div>
  <div class="content">{@render children()}</div>
</div>

<style>
  .scene {
    position: relative;
    overflow: hidden;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: #6f6b67;
    isolation: isolate;
  }
  .no-cursor {
    cursor: none;
  }
  .sky {
    position: absolute;
    inset: 0 0 auto;
    height: 34%;
    background: linear-gradient(180deg, #b9cbd6, #dfe7eb);
  }
  .floor {
    position: absolute;
    left: -60%;
    right: -60%;
    top: 34%;
    height: 140%;
    transform-origin: 50% 0;
    transform: perspective(260px) rotateX(58deg);
    background-color: #6f6b67;
    background-image:
      radial-gradient(circle at 50% 50%, transparent 0 7px, rgba(0, 0, 0, 0) 7px),
      linear-gradient(rgba(255, 255, 255, 0.07) 0 2px, transparent 2px),
      linear-gradient(90deg, rgba(0, 0, 0, 0.16) 0 2px, transparent 2px),
      linear-gradient(rgba(0, 0, 0, 0.16) 0 2px, transparent 2px);
    background-size: 26px 26px;
    background-position:
      0 0,
      0 3px,
      0 0,
      0 0;
  }
  .content {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
</style>
