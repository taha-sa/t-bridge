<script lang="ts">
  let { checked, onchange, label }: { checked: boolean; onchange: (v: boolean) => void; label?: string } = $props()
</script>

<button class="tg" class:on={checked} role="switch" aria-checked={checked} aria-label={label} onclick={() => onchange(!checked)}>
  <span class="knob"></span>
</button>

<style>
  .tg {
    width: 42px;
    height: 24px;
    padding: 0;
    border-radius: 99px;
    background: var(--s4);
    border: 1px solid var(--line-2);
    position: relative;
    flex: none;
  }
  .tg:hover:not(:disabled) {
    background: var(--s4);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #d8d5cc;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    transition:
      transform 240ms var(--ease),
      background 240ms var(--ease);
  }
  .on,
  .on:hover:not(:disabled) {
    background: var(--gold-grad);
    border-color: transparent;
  }
  .on .knob {
    transform: translateX(18px);
    background: #fff8e6;
  }
</style>
