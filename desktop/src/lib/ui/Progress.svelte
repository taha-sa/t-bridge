<script lang="ts">
  // İlerleme çubuğu; aktifken üzerinde parıltı akar.
  let { value, active = false, tone = 'gold' }: { value: number; active?: boolean; tone?: 'gold' | 'ok' | 'bad' | 'muted' } = $props()
</script>

<div class="bar {tone}" class:active>
  <div class="fill" style="width:{Math.max(0, Math.min(100, value))}%"></div>
</div>

<style>
  .bar {
    height: 6px;
    border-radius: 99px;
    background: var(--s4);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 99px;
    background: var(--gold-grad);
    transition: width 300ms linear;
    position: relative;
    overflow: hidden;
  }
  .ok .fill {
    background: var(--ok);
  }
  .bad .fill {
    background: var(--bad);
  }
  .muted .fill {
    background: var(--faint);
  }
  .active .fill::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.45), transparent);
    animation: sheen 1.4s linear infinite;
  }
  @keyframes sheen {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(100%);
    }
  }
</style>
