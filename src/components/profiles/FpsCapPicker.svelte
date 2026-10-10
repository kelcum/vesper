<script lang="ts">
  import { FPS_PRESETS } from '../../lib/format';
  import type { FpsCap } from '../../lib/types';

  interface Props {
    value: FpsCap | null;
    displayHz: number | null;
    onchange: (cap: FpsCap | null) => void;
  }
  let { value, displayHz, onchange }: Props = $props();

  let custom = $state('');
  // Mirror a custom (non-preset) limit into the field, including when the profile changes.
  $effect(() => {
    custom = value?.mode === 'fixed' && !FPS_PRESETS.includes(value.value as never) ? String(value.value) : '';
  });
  const customError = $derived(custom && (Number(custom) < 15 || Number(custom) > 1000 || !/^\d+$/.test(custom)));

  function isSel(cap: FpsCap | null): boolean {
    if (cap === null || value === null) return cap === value;
    if (cap.mode !== value.mode) return false;
    return cap.mode !== 'fixed' || (value.mode === 'fixed' && cap.value === value.value);
  }

  const customSelected = $derived(value?.mode === 'fixed' && !FPS_PRESETS.includes(value.value as never));

  function applyCustom() {
    if (!custom || customError) return;
    onchange({ mode: 'fixed', value: Number(custom) });
  }
</script>

<div class="picker">
  <div class="chips" role="radiogroup" aria-label="Frame-rate limit">
    <button role="radio" aria-checked={isSel(null)} class:sel={isSel(null)} onclick={() => onchange(null)}>Don't change</button>
    {#each FPS_PRESETS as fps (fps)}
      {@const cap: FpsCap = { mode: 'fixed', value: fps }}
      <button role="radio" aria-checked={isSel(cap)} class:sel={isSel(cap)} onclick={() => onchange(cap)}>{fps}</button>
    {/each}
    <button
      role="radio"
      aria-checked={isSel({ mode: 'matchDisplay' })}
      class:sel={isSel({ mode: 'matchDisplay' })}
      onclick={() => onchange({ mode: 'matchDisplay' })}>Match display{displayHz ? ` · ${displayHz}` : ''}</button
    >
    <button role="radio" aria-checked={isSel({ mode: 'unlimited' })} class:sel={isSel({ mode: 'unlimited' })} onclick={() => onchange({ mode: 'unlimited' })}>Unlimited</button>
    <button role="radio" aria-checked={isSel({ mode: 'robloxDefault' })} class:sel={isSel({ mode: 'robloxDefault' })} onclick={() => onchange({ mode: 'robloxDefault' })}>Roblox default</button>
  </div>
  <div class="custom" class:sel={customSelected}>
    <label for="fps-custom">Custom</label>
    <input
      id="fps-custom"
      inputmode="numeric"
      placeholder="e.g. 100"
      bind:value={custom}
      class:invalid={customError}
      onkeydown={(e) => e.key === 'Enter' && applyCustom()}
      onblur={applyCustom}
    />
    <span class="faint small">FPS (15–1000)</span>
  </div>
  <p class="faint small">
    Sets Roblox's own frame-rate cap before it starts. A limit is a ceiling, not a promise: Roblox still renders as fast as your PC
    allows below it. “Unlimited” writes a cap of 9999.
  </p>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  button,
  .custom {
    height: 30px;
    padding: 0 11px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--text-sm);
    font-weight: 550;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  button:hover {
    color: var(--text);
    border-color: var(--border-strong);
  }
  .sel {
    color: var(--text);
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .custom {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    cursor: default;
  }
  .custom input {
    width: 70px;
    height: 22px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-2);
    font-family: var(--font-mono);
  }
  .custom input.invalid {
    border-color: var(--bad);
  }
</style>
