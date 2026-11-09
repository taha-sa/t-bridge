<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { flash } from './store.svelte'
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

  const open = (c: Card) => run(c.id, () => invoke('suite_open', { id: c.id }), c.running ? 'Öne getirildi' : `${c.name} açılıyor`)
  const update = (c: Card) => run(c.id, () => invoke('suite_update', { id: c.id }), `${c.name} güncelleniyor`)
  const updateAll = () => run('all', () => invoke('suite_update', { id: null }), 'Güncellemeler başlatıldı')
  const check = () => run('check', () => invoke('suite_check'), 'Güncellemeler denetlendi')
  const releases = (c: Card) => invoke('open_url', { url: c.releasesUrl }).catch(() => {})

  onMount(() => {
    refresh()
    const t = setInterval(refresh, 2000)
    return () => clearInterval(t)
  })
</script>

<div class="page">
  <div class="row headrow">
    <div class="grow">
      <div class="muted small">T-Suite uygulamaları bu bilgisayarda ve telefonunda tek yerden yönetilir.</div>
    </div>
    <button class="ghost sm" disabled={busy !== null} onclick={check}><Icon name="refresh" size={15} /> Denetle</button>
    <button class="primary sm" disabled={busy !== null || updatable === 0} onclick={updateAll}>Hepsini güncelle{updatable ? ` (${updatable})` : ''}</button>
  </div>

  {#if cards === null}
    <section class="card tight loading muted small">Uygulamalar aranıyor…</section>
  {:else if cards.length === 0}
    <section class="card tight muted small">Henüz kayıtlı T-Suite uygulaması yok.</section>
  {:else}
    <div class="grid">
      {#each cards as c (c.id)}
        <article class="card app" class:off={!c.installed}>
          <header class="row top">
            <div class="ico"><Icon name={ICONS[c.id] ?? 'sparkle'} size={20} /></div>
            <div class="grow">
              <div class="nm ellipsis">{c.name}</div>
              <div class="muted small">{c.installed ? `Sürüm ${c.version || '—'}` : 'Yüklü değil'}</div>
            </div>
            {#if c.installed}
              <span class="st" class:on={c.running} title={c.running ? 'Çalışıyor' : 'Kapalı'}><span class="dot"></span>{c.running ? 'Açık' : 'Kapalı'}</span>
            {/if}
          </header>

          <div class="badgeslot">
            {#if c.updateAvailable}<span class="badge">Güncelleme var · {c.updateAvailable}</span>{/if}
          </div>

          <footer class="row acts">
            {#if c.installed}
              <button class="sm" disabled={busy !== null} onclick={() => open(c)}>{c.running ? 'Öne getir' : 'Aç'}</button>
              {#if c.updateAvailable}
                <button class="primary sm" disabled={busy !== null} onclick={() => update(c)}>Güncelle</button>
              {/if}
            {:else}
              <button class="ghost sm" onclick={() => releases(c)}>Sürümler sayfası</button>
            {/if}
          </footer>
        </article>
      {/each}
    </div>
  {/if}
</div>

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
