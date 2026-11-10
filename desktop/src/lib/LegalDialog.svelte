<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { fade, scale } from 'svelte/transition'
  import { t, i18n } from './i18n/index.svelte'
  import { renderMd } from './legal/md'
  import notices from './legal/notices.json'
  import license from './legal/LICENSE.txt?raw'

  let { kind, onclose }: { kind: 'privacy' | 'disclaimer' | 'license' | 'notices'; onclose: () => void } = $props()

  const docs = import.meta.glob('./legal/*.{tr,en,de}.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>
  const title = $derived(
    kind === 'privacy' ? t('about.privacy') : kind === 'disclaimer' ? t('about.disclaimer') : kind === 'license' ? t('about.license') : t('about.notices'),
  )
  const html = $derived(
    kind === 'privacy' || kind === 'disclaimer' ? renderMd(docs[`./legal/${kind}.${i18n.lang}.md`] ?? docs[`./legal/${kind}.en.md`] ?? '') : '',
  )
  const rows = notices as [string, string, string, string][]

  function onClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest('a[data-url]')
    if (a) {
      e.preventDefault()
      invoke('open_url', { url: a.getAttribute('data-url') }).catch(() => {})
    }
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" transition:fade={{ duration: 140 }} onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="lg-title" in:scale={{ start: 0.97, duration: 180 }}>
    <header>
      <h2 id="lg-title">{title}</h2>
      <button class="ghost sm" onclick={onclose}>{t('common.close')}</button>
    </header>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="body" onclick={onClick} role="document">
      {#if kind === 'license'}
        <pre>{license}</pre>
      {:else if kind === 'notices'}
        <p class="muted small">{t('about.noticesCount', { n: rows.length })}</p>
        <table>
          <tbody>
            {#each rows as r (r[3] + r[0] + r[1])}
              <tr><td>{r[0]}</td><td class="faint">{r[1]}</td><td>{r[2]}</td></tr>
            {/each}
          </tbody>
        </table>
      {:else}
        {@html html}
      {/if}
    </div>
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
    width: min(680px, 100%);
    max-height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-xl);
    background: var(--s1);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .body {
    overflow: auto;
    padding: 8px 20px 20px;
    line-height: 1.55;
    user-select: text;
  }
  .body :global(h4) {
    margin: 18px 0 6px;
    font-size: 13px;
  }
  .body :global(p),
  .body :global(li) {
    color: var(--text-2);
    margin: 6px 0;
  }
  .body :global(ul),
  .body :global(ol) {
    padding-left: 20px;
    margin: 6px 0;
  }
  .body :global(a) {
    color: var(--gold);
  }
  pre {
    white-space: pre-wrap;
    font: inherit;
    color: var(--text-2);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  td {
    padding: 5px 8px 5px 0;
    border-bottom: 1px solid var(--line);
    vertical-align: top;
  }
</style>
