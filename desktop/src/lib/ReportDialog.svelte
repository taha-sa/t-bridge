<script lang="ts" module>
  export interface Preview {
    text: string
    url: string
    title: string
  }
</script>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { fade, scale } from 'svelte/transition'
  import { t } from './i18n/index.svelte'
  import { flash } from './store.svelte'

  // Çökme sonrası (manual=false) ya da elle hazırlanan rapor. Hiçbir şey kendiliğinden gönderilmez.
  let { preview, manual, onclose }: { preview: Preview; manual: boolean; onclose: () => void } = $props()

  async function finish() {
    if (!manual) await invoke('crash_dismiss').catch(() => {})
    onclose()
  }
  async function send() {
    await invoke('open_url', { url: preview.url }).catch(() => {})
    await finish()
  }
  async function copy() {
    try {
      await navigator.clipboard.writeText(preview.text)
      flash(t('crash.copied'))
    } catch {
      /* pano erişilemedi */
    }
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') finish()
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" transition:fade={{ duration: 140 }} onclick={(e) => e.target === e.currentTarget && finish()}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="cr-title" in:scale={{ start: 0.97, duration: 180 }}>
    <h2 id="cr-title">{manual ? t('crash.manualTitle') : t('crash.title')}</h2>
    <p class="muted">{manual ? t('crash.bodyManual') : t('crash.body')}</p>
    <textarea readonly spellcheck="false" value={preview.text} aria-label={t('crash.manualTitle')}></textarea>
    <div class="actions">
      <button class="ghost" onclick={finish}>{t('common.close')}</button>
      <button class="ghost" onclick={copy}>{t('crash.copy')}</button>
      <button class="primary" onclick={send}>{t('crash.send')}</button>
    </div>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(5, 5, 6, 0.66);
    backdrop-filter: blur(5px);
  }
  .dlg {
    width: min(640px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 22px 22px 18px;
    border-radius: var(--radius-xl);
    background: var(--s1);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  p {
    margin: 0;
  }
  textarea {
    min-height: 220px;
    flex: 1;
    resize: none;
    padding: 10px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-2);
    font: 11.5px/1.45 Consolas, 'Cascadia Mono', monospace;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
