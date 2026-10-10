<script lang="ts">
  // The emote wheel as Roblox draws it, built from the exact files Vesper writes
  // (Roblox's "Large" size at 1x: a 400 px wheel). Point at a slot to see its
  // highlight, as you would in game.
  interface Props {
    background: string | null;
    ring: string | null;
    gradient: string | null;
    line: string | null;
    size?: number;
  }
  let { background, ring, gradient, line, size = 210 }: Props = $props();

  const BASE = 400;
  const k = $derived(size / BASE);
  let slot = $state(0);
  let wheel: HTMLDivElement | undefined = $state();
  let gw = $state(0);
  let gh = $state(0);
  let lw = $state(0);
  let lh = $state(0);

  const slots = Array.from({ length: 8 }, (_, i) => {
    const a = (i * 45 * Math.PI) / 180;
    return { n: i + 1, x: 50 + Math.sin(a) * 27.75, y: 50 - Math.cos(a) * 27.75 };
  });

  function point(e: PointerEvent) {
    if (!wheel) return;
    const r = wheel.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2);
    const dy = e.clientY - (r.top + r.height / 2);
    if (Math.hypot(dx, dy) < 12) return;
    const deg = (Math.atan2(dx, -dy) * 180) / Math.PI;
    slot = (Math.round(((deg + 360) % 360) / 45) % 8) as number;
  }
</script>

<div
  class="wheel"
  bind:this={wheel}
  style:width="{size}px"
  style:height="{size}px"
  onpointermove={point}
  role="img"
  aria-label="Preview of the emote wheel, slot {slot + 1} highlighted"
>
  {#if background}<img class="layer" src={background} alt="" />{/if}
  <!-- Roblox points the highlight at the slot under the mouse; it starts pointing right. -->
  <div class="spin" style:transform="rotate({slot * 45 - 90}deg)">
    {#if gradient}
      <img
        class="gradient"
        src={gradient}
        alt=""
        bind:naturalWidth={gw}
        bind:naturalHeight={gh}
        style:width="{gw * k}px"
        style:height="{gh * k}px"
      />
    {/if}
  </div>
  {#if ring}<img class="layer" src={ring} alt="" />{/if}
  <div class="spin" style:transform="rotate({slot * 45 - 90}deg)">
    {#if line}
      <img
        class="line"
        src={line}
        alt=""
        bind:naturalWidth={lw}
        bind:naturalHeight={lh}
        style:width="{lw * k}px"
        style:height="{lh * k}px"
      />
    {/if}
  </div>
  {#each slots as s (s.n)}
    <span class="num" style:left="{s.x}%" style:top="{s.y}%" style:font-size="{size * 0.05}px">{s.n}</span>
  {/each}
  <span class="title" style:font-size="{size * 0.052}px">Select an Emote</span>
</div>

<style>
  .wheel {
    position: relative;
    flex: none;
  }
  .layer,
  .spin {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .spin {
    transition: transform 140ms var(--ease-out);
  }
  .gradient {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translateY(-50%);
  }
  .line {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
  }
  .num,
  .title {
    position: absolute;
    transform: translate(-50%, -50%);
    color: #fff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
    font-weight: 500;
    pointer-events: none;
  }
  .title {
    left: 50%;
    top: 50%;
    white-space: nowrap;
  }
</style>
