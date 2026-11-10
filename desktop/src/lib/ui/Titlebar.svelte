<script lang="ts">
  // Özel başlık çubuğu: sürükleme bölgesi + küçült / büyüt / kapat (kapat = tepsiye).
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { t } from '../i18n/index.svelte'
  const w = getCurrentWindow()
  let { title = '' }: { title?: string } = $props()
</script>

<!-- Çift tıklama ile büyütmeyi Tauri sürükleme bölgesi kendisi yapar. -->
<header class="tb" data-tauri-drag-region>
  <div class="t" data-tauri-drag-region>{title}</div>
  <div class="ctl">
    <button class="wb" aria-label={t('title.min')} onclick={() => w.minimize()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 5h10" /></svg>
    </button>
    <button class="wb" aria-label={t('title.max')} onclick={() => w.toggleMaximize()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><rect x="0.5" y="0.5" width="9" height="9" rx="1.5" /></svg>
    </button>
    <button class="wb x" aria-label={t('title.closeAria')} title={t('title.closeTitle')} onclick={() => w.close()}>
      <svg viewBox="0 0 10 10" width="10" height="10"><path d="M0 0l10 10M10 0L0 10" /></svg>
    </button>
  </div>
</header>

<style>
  .tb {
    height: 38px;
    flex: none;
    display: flex;
    align-items: center;
    padding-left: 16px;
    position: relative;
    z-index: 5;
  }
  .t {
    flex: 1;
    font-size: 12px;
    color: var(--muted);
    letter-spacing: 0.02em;
    height: 100%;
    display: flex;
    align-items: center;
  }
  .ctl {
    display: flex;
    height: 100%;
  }
  .wb {
    width: 46px;
    height: 100%;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--text-2);
  }
  .wb:active:not(:disabled) {
    transform: none;
  }
  .wb svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
  .wb:hover:not(:disabled) {
    background: var(--s3);
    border: none;
  }
  .x:hover:not(:disabled) {
    background: #c42b1c;
    color: #fff;
  }
</style>
