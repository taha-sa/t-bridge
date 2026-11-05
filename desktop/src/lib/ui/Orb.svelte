<script lang="ts">
  // Bağlantı küresi: bağlıyken altın nefes, bağlanırken dönen halka, bağlı değilken sönük.
  import type { ConnState } from '../types'
  let { state, size = 56 }: { state: ConnState; size?: number } = $props()
</script>

<div class="orb" class:on={state === 'Connected'} class:mid={state === 'Connecting'} style="--sz:{size}px">
  <div class="halo"></div>
  <div class="ring"></div>
  <div class="core"></div>
</div>

<style>
  .orb {
    position: relative;
    width: var(--sz);
    height: var(--sz);
    flex: none;
  }
  .core {
    position: absolute;
    inset: 30%;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, #6b6962, #2b2a27 70%);
    box-shadow: inset 0 1px 2px rgba(255, 255, 255, 0.15);
    transition: background 400ms var(--ease);
  }
  .halo {
    position: absolute;
    inset: 12%;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(143, 140, 132, 0.12), transparent 70%);
  }
  .ring {
    position: absolute;
    inset: 4%;
    border-radius: 50%;
    border: 1.5px solid var(--line-2);
  }
  .on .core {
    background: radial-gradient(circle at 35% 30%, #ffe08a, #e3a92b 55%, #8a5a00);
    box-shadow:
      0 0 18px rgba(245, 197, 66, 0.55),
      inset 0 1px 2px rgba(255, 255, 255, 0.5);
  }
  .on .halo {
    background: radial-gradient(circle, rgba(245, 197, 66, 0.35), transparent 70%);
    animation: breathe 3.2s ease-in-out infinite;
  }
  .on .ring {
    border-color: var(--gold-line);
  }
  .mid .core {
    background: radial-gradient(circle at 35% 30%, #c9b27a, #7d6a3e 70%);
  }
  .mid .ring {
    border-color: transparent;
    border-top-color: var(--gold);
    border-right-color: rgba(227, 169, 43, 0.35);
    animation: spin 1.1s linear infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      transform: scale(0.92);
      opacity: 0.75;
    }
    50% {
      transform: scale(1.18);
      opacity: 1;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
