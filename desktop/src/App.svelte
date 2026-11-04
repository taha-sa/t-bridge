<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { app, call, flash, init } from './lib/store.svelte'
  import Icon from './lib/Icon.svelte'
  import Home from './lib/Home.svelte'
  import Files from './lib/Files.svelte'
  import Notifs from './lib/Notifs.svelte'
  import Remote from './lib/Remote.svelte'
  import SettingsPage from './lib/SettingsPage.svelte'

  type Tab = 'home' | 'files' | 'notifs' | 'remote' | 'settings'
  let tab = $state<Tab>('home')
  let dragging = $state(false)

  const s = $derived(app.s)
  const active = $derived(s.transfers.filter((t) => t.state === 'Active' || t.state === 'Queued').length)

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: 'home', label: 'Bağlantı', icon: 'home' },
    { id: 'files', label: 'Dosyalar', icon: 'files' },
    { id: 'notifs', label: 'Bildirimler', icon: 'bell' },
    { id: 'remote', label: 'Kumanda', icon: 'remote' },
    { id: 'settings', label: 'Ayarlar', icon: 'gear' },
  ]

  onMount(() => {
    init()
    let unlisten: (() => void) | undefined
    getCurrentWebview()
      .onDragDropEvent(async (e) => {
        const p = e.payload
        if (p.type === 'enter' || p.type === 'over') {
          dragging = true
          if (tab !== 'files') tab = 'files'
        } else if (p.type === 'leave') {
          dragging = false
        } else if (p.type === 'drop') {
          dragging = false
          if (!p.paths.length) return
          if (app.s.peers.length === 0) {
            flash('Önce bir telefon eşleştir')
            return
          }
          const n = await call<number>('send_paths', { paths: p.paths })
          if (n) flash(`${n} dosya sıraya eklendi`)
        }
      })
      .then((u) => (unlisten = u))
    return () => unlisten?.()
  })
</script>

<div class="shell">
  <nav>
    <div class="brand">Köprü</div>
    {#each tabs as t (t.id)}
      <button class="navbtn" class:sel={tab === t.id} onclick={() => (tab = t.id)}>
        <Icon name={t.icon} />
        <span class="grow">{t.label}</span>
        {#if t.id === 'files' && active}<span class="count">{active}</span>{/if}
        {#if t.id === 'notifs' && s.notifications.length}<span class="count">{s.notifications.length}</span>{/if}
      </button>
    {/each}
    <div class="grow"></div>
    <div class="status" class:on={s.state === 'Connected'}>
      <span class="dot"></span>
      <span class="ellipsis">
        {s.state === 'Connected' ? (s.peer?.name ?? 'Bağlı') : s.state === 'Connecting' ? 'Bağlanıyor…' : 'Bağlı değil'}
      </span>
      {#if s.state === 'Connected' && s.battery}<span class="muted">%{s.battery.level}{s.battery.charging ? '⚡' : ''}</span>{/if}
    </div>
  </nav>

  <main>
    {#if !app.ready}
      <div class="muted">Yükleniyor…</div>
    {:else if tab === 'home'}
      <Home />
    {:else if tab === 'files'}
      <Files {dragging} />
    {:else if tab === 'notifs'}
      <Notifs />
    {:else if tab === 'remote'}
      <Remote />
    {:else}
      <SettingsPage />
    {/if}
  </main>

  {#if app.toast}
    <div class="toast">{app.toast}</div>
  {/if}
</div>

<style>
  .shell {
    display: flex;
    height: 100%;
  }
  nav {
    width: 210px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }
  .brand {
    font-size: 18px;
    font-weight: 700;
    padding: 4px 10px 16px;
    letter-spacing: 0.3px;
  }
  .navbtn {
    border: none;
    background: transparent;
    justify-content: flex-start;
    padding: 9px 10px;
    width: 100%;
    text-align: left;
  }
  .navbtn.sel {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .count {
    font-size: 11px;
    background: var(--accent);
    color: #fff;
    border-radius: 99px;
    padding: 0 7px;
    min-width: 20px;
    text-align: center;
  }
  .status {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px;
    font-size: 12px;
    border-top: 1px solid var(--line);
  }
  .status .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
  }
  .status.on .dot {
    background: var(--ok);
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 22px 26px;
  }
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 9px 16px;
    border-radius: 10px;
    font-size: 13px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.2);
    z-index: 10;
  }
</style>
