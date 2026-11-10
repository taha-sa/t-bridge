<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { flash } from './store.svelte'
  import { t, tn } from './i18n/index.svelte'
  import { isAppId, type AppId } from './apps'
  import InviteDialog from './InviteDialog.svelte'
  import Icon from './Icon.svelte'

  interface Card {
    id: string
    name: string
    version: string
    installed: boolean
    running: boolean
    updateAvailable: string | null
    releasesUrl: string
  }

  const ICONS: Record<string, string> = { 't-ramcash': 'battery', 't-downloader': 'download', 't-desktree': 'folder', 't-bridge': 'phone' }

  let cards = $state<Card[] | null>(null)
  let busy = $state<string | null>(null)
  /** Açık davet (yalnızca bu örnek; kapatınca tekrar basılana kadar görünmez). */
  let invite = $state<AppId | null>(null)
  const updatable = $derived((cards ?? []).filter((c) => c.updateAvailable).length)

  async function refresh() {
    try {
      cards = await invoke<Card[]>('suite_status')
    } catch {
      /* hub henüz hazır değil */
    }
  }

  async function run(key: string, fn: () => Promise<unknown>, ok: string) {
    busy = key
    try {
      await fn()
      flash(ok)
    } catch (e) {
      flash(String(e))
    } finally {
      busy = null
      refresh()
    }
  }

  // Uygulama yoksa düğme yine görünür: basınca o uygulamaya özel davet açılır.
  function open(c: Card) {
    if (!c.installed) {
      if (isAppId(c.id)) invite = c.id
      else if (c.releasesUrl) invoke('open_url', { url: c.releasesUrl }).catch(() => {})
      return
    }
    return run(c.id, () => invoke('suite_open', { id: c.id }), c.running ? t('suite.focused') : t('suite.opening', { name: c.name }))
  }
  const update = (c: Card) => run(c.id, () => invoke('suite_update', { id: c.id }), t('suite.updating', { name: c.name }))
  const updateAll = () => run('all', () => invoke('suite_update', { id: null }), t('suite.updatesStarted'))
  const check = () => run('check', () => invoke('suite_check'), t('suite.checked'))

  onMount(() => {
    refresh()
    const timer = setInterval(refresh, 2000)
    return () => clearInterval(timer)
  })
</script>

<div class="page">
  <div class="row headrow">
    <div class="grow">
      <div class="muted small">{t('suite.intro')}</div>
    </div>
    <button class="ghost sm" disabled={busy !== null} onclick={check}><Icon name="refresh" size={15} /> {t('suite.check')}</button>
    <button class="primary sm" disabled={busy !== null || updatable === 0} onclick={updateAll}>{t('suite.updateAll')}{updatable ? ` (${updatable})` : ''}</button>
  </div>

  {#if cards === null}
    <section class="card tight loading muted small">{t('suite.searching')}</section>
  {:else if cards.length === 0}
    <section class="card tight muted small">{t('suite.none')}</section>
  {:else}
    <div class="grid">
      {#each cards as c (c.id)}
        <article class="card app" class:off={!c.installed}>
          <header class="row top">
            <div class="ico"><Icon name={ICONS[c.id] ?? 'sparkle'} size={20} /></div>
            <div class="grow">
              <div class="nm ellipsis">{c.name}</div>
              <div class="muted small">{c.installed ? t('suite.version', { v: c.version || '—' }) : t('suite.notInstalled')}</div>
            </div>
            {#if c.installed}
              <span class="st" class:on={c.running} title={c.running ? t('suite.runningTitle') : t('suite.closed')}><span class="dot"></span>{c.running ? t('suite.on') : t('suite.closed')}</span>
            {/if}
          </header>

          <div class="badgeslot">
            {#if c.updateAvailable}<span class="badge">{t('suite.updateBadge', { v: c.updateAvailable })}</span>{/if}
          </div>

          <footer class="row acts">
            <button class="sm" disabled={busy !== null} onclick={() => open(c)}>{c.installed && c.running ? t('suite.focus') : t('common.open')}</button>
            {#if c.installed && c.updateAvailable}
              <button class="primary sm" disabled={busy !== null} onclick={() => update(c)}>{t('suite.update')}</button>
            {/if}
          </footer>
        </article>
      {/each}
    </div>
  {/if}
</div>

{#if invite}
  <InviteDialog app={invite} onclose={() => (invite = null)} />
{/if}

<style>
  .headrow {
    gap: 8px;
    min-height: 32px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }
  .app {
    min-height: 148px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition:
      transform 150ms var(--ease),
      box-shadow 150ms var(--ease),
      border-color 150ms var(--ease);
  }
  .app:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow);
    border-color: var(--gold-line);
  }
  .app.off {
    opacity: 0.6;
  }
  .top {
    gap: 12px;
    align-items: center;
  }
  .ico {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  .nm {
    font-weight: 650;
  }
  .st {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .st .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .st.on {
    color: var(--ok);
  }
  .st.on .dot {
    background: var(--ok);
  }
  .badgeslot {
    min-height: 24px; /* rozet yokken de yer ayrılır: düzen kaymaz */
  }
  .badge {
    display: inline-block;
    padding: 2px 10px;
    border-radius: 99px;
    font-size: 12px;
    font-weight: 650;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  .acts {
    margin-top: auto;
    gap: 8px;
  }
  .loading {
    min-height: 72px;
    display: grid;
    place-items: center;
  }
  @media (prefers-reduced-motion: reduce) {
    .app {
      transition: none;
    }
    .app:hover {
      transform: none;
    }
  }
</style>
