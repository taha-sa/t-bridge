<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { fade, scale } from 'svelte/transition'
  import { t, type Key } from './i18n/index.svelte'
  import { APPS, type AppId } from './apps'

  // Eksik uygulama daveti: özelliğin düğmesi her zaman görünür; × yalnızca bu mesajı kapatır (kalıcı gizleme yok).
  let { app, onclose }: { app: AppId; onclose: () => void } = $props()

  const info = $derived(APPS[app])
  const k = (part: string) => `invite.${app}.${part}` as Key

  function get() {
    invoke('open_url', { url: info.url }).catch(() => {})
    onclose()
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" transition:fade={{ duration: 140 }} onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="inv-title" style="--acc: {info.accent}" in:scale={{ start: 0.96, duration: 200 }}>
    <button class="x ghost icon sm" aria-label={t('common.close')} onclick={onclose}>
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
    <img class="ic" src={info.icon} alt="" width="64" height="64" />
    <h2 id="inv-title">{t(k('title'))}</h2>
    <p class="muted">{t(k('body'))}</p>
    <button class="cta" onclick={get}>{t(k('cta'))}</button>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(5, 5, 6, 0.66);
    backdrop-filter: blur(5px);
  }
  .dlg {
    position: relative;
    width: min(400px, 100%);
    padding: 30px 28px 26px;
    border-radius: 22px;
    background: var(--bg-2);
    border: 1px solid color-mix(in srgb, var(--acc) 38%, transparent);
    box-shadow:
      0 24px 60px -18px rgba(0, 0, 0, 0.7),
      0 0 0 1px rgba(255, 255, 255, 0.03) inset;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 10px;
  }
  .x {
    position: absolute;
    top: 10px;
    right: 10px;
  }
  .ic {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    margin-bottom: 4px;
    box-shadow: 0 10px 28px -10px var(--acc);
  }
  h2 {
    font-size: 18px;
    font-weight: 650;
    margin: 0;
  }
  p {
    margin: 0 0 8px;
    line-height: 1.5;
  }
  .cta {
    min-height: 40px;
    padding: 0 22px;
    border: 0;
    border-radius: 12px;
    font-weight: 650;
    color: #1a1305;
    background: var(--acc);
    cursor: pointer;
  }
  .cta:hover {
    filter: brightness(1.08);
  }
</style>
