<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    disabled?: boolean;
    onchange?: (value: boolean) => void;
  }
  let { checked = $bindable(), label, disabled = false, onchange }: Props = $props();

  function flip() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  class="toggle"
  class:on={checked}
  {disabled}
  onclick={flip}
>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    flex: none;
    width: 40px;
    height: 23px;
    padding: 0;
    border-radius: var(--radius-pill);
    border: 1px solid var(--border-strong);
    background: var(--surface-3);
    cursor: pointer;
    transition:
      background var(--dur) var(--ease-out),
      border-color var(--dur) var(--ease-out);
  }
  .toggle.on {
    background: var(--accent-gradient);
    border-color: transparent;
  }
  .toggle:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
    transition: transform var(--dur) var(--ease-spring);
  }
  .on .knob {
    transform: translateX(17px);
  }
  .toggle:active:not(:disabled) .knob {
    width: 20px;
  }
  .on:active:not(:disabled) .knob {
    transform: translateX(14px);
  }
</style>
