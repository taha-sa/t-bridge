<script lang="ts">
  // Pil halkası: yüzde, şarj animasyonu; %15 altı kırmızı.
  import { t } from '../i18n/index.svelte'
  let { level, charging, size = 46 }: { level: number; charging: boolean; size?: number } = $props()
  const r = 19
  const c = 2 * Math.PI * r
  const low = $derived(level <= 15 && !charging)
  const off = $derived(c * (1 - Math.max(0, Math.min(100, level)) / 100))
</script>

<div class="bat" class:low class:charging style="--sz:{size}px" title={t('battery.title', { level }) + (charging ? ', ' + t('battery.charging') : '')}>
  <svg viewBox="0 0 46 46" width={size} height={size}>
    <circle cx="23" cy="23" r={r} class="track" />
    <circle cx="23" cy="23" r={r} class="val" stroke-dasharray={c} stroke-dashoffset={off} />
  </svg>
  <div class="lbl num">
    {#if charging}<svg class="bolt" viewBox="0 0 24 24" width="10" height="10"><path d="M13 2L4 14h7l-1 8 9-12h-7z" /></svg>{/if}{level}
  </div>
</div>

<style>
  .bat {
    position: relative;
    width: var(--sz);
    height: var(--sz);
    flex: none;
  }
  svg {
    transform: rotate(-90deg);
  }
  circle {
    fill: none;
    stroke-width: 3.5;
  }
  .track {
    stroke: var(--s4);
  }
  .val {
    stroke: var(--gold);
    stroke-linecap: round;
    transition: stroke-dashoffset 600ms var(--ease);
  }
  .low .val {
    stroke: var(--bad);
  }
  .charging .val {
    stroke: var(--ok);
    animation: pulse 2s ease-in-out infinite;
  }
  .lbl {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    font-size: 12px;
    font-weight: 650;
  }
  .lbl svg {
    transform: none;
  }
  .bolt {
    fill: var(--ok);
  }
  @keyframes pulse {
    50% {
      opacity: 0.55;
    }
  }
</style>
