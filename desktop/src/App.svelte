<script lang="ts">
  import { onMount } from 'svelte'
  import { fade, fly, scale } from 'svelte/transition'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { app, call, flash, init } from './lib/store.svelte'
  import { t, tn, type Key } from './lib/i18n/index.svelte'
  import Icon from './lib/Icon.svelte'
  import Logo from './lib/ui/Logo.svelte'
  import Orb from './lib/ui/Orb.svelte'
  import BatteryRing from './lib/ui/BatteryRing.svelte'
  import Titlebar from './lib/ui/Titlebar.svelte'
  import Home from './lib/Home.svelte'
  import Files from './lib/Files.svelte'
  import Notifs from './lib/Notifs.svelte'
  import Remote from './lib/Remote.svelte'
  import Camera from './lib/Camera.svelte'
  import Suite from './lib/Suite.svelte'
  import SettingsPage from './lib/SettingsPage.svelte'
  import ReportDialog, { type Preview } from './lib/ReportDialog.svelte'
  import { invoke } from '@tauri-apps/api/core'

  type Tab = 'home' | 'files' | 'notifs' | 'remote' | 'camera' | 'suite' | 'settings'
  let tab = $state<Tab>('home')
  let dragging = $state(false)
  let mainEl = $state<HTMLElement>()
  $effect(() => {
    void tab
    mainEl?.scrollTo({ top: 0 })
  })

  const s = $derived(app.s)
  const active = $derived(s.transfers.filter((x) => x.state === 'Active' || x.state === 'Queued').length)
  const on = $derived(s.state === 'Connected')

  const tabs: { id: Tab; icon: string }[] = [
    { id: 'home', icon: 'home' },
    { id: 'files', icon: 'files' },
    { id: 'notifs', icon: 'bell' },
    { id: 'remote', icon: 'remote' },
    { id: 'camera', icon: 'video' },
    { id: 'suite', icon: 'sparkle' },
    { id: 'settings', icon: 'gear' },
  ]
  const tabLabel = (id: Tab) => t(`nav.${id}` as Key)

  // Önceki oturumdan kalan çökme raporu varsa açılışta göster.
  let crash = $state<Preview | null>(null)

  onMount(() => {
    init()
    invoke<Preview | null>('crash_pending')
      .then((p) => (crash = p))
      .catch(() => {})
    let unlisten: (() => void) | undefined
    getCurrentWebview()
      .onDragDropEvent(async (e) => {
        const p = e.payload
        if (p.type === 'enter' || p.type === 'over') {
          dragging = true
        } else if (p.type === 'leave') {
          dragging = false
        } else if (p.type === 'drop') {
          dragging = false
          if (!p.paths.length) return
          if (app.s.peers.length === 0) {
            flash(t('app.needPhone'))
            return
          }
          tab = 'files'
          const n = await call<number>('send_paths', { paths: p.paths })
          if (n) flash(tn('files.queued', n))
        }
      })
      .then((u) => (unlisten = u))
    return () => unlisten?.()
  })
</script>

<div class="shell">
  <nav>
    <div class="brand" data-tauri-drag-region>
      <Logo size={30} />
      <div class="bt" data-tauri-drag-region>
        <div class="name">T-Bridge</div>
        <div class="ver faint">{s.version ? `v${s.version}` : ''}</div>
      </div>
    </div>

    <div class="tabs">
      {#each tabs as tb (tb.id)}
        <button class="navbtn" class:sel={tab === tb.id} onclick={() => (tab = tb.id)}>
          <span class="ico"><Icon name={tb.icon} size={18} /></span>
          <span class="grow">{tabLabel(tb.id)}</span>
          {#if tb.id === 'files' && active}<span class="count num">{active}</span>{/if}
          {#if tb.id === 'notifs' && s.notifications.length}<span class="count violet num">{s.notifications.length}</span>{/if}
          {#if tb.id === 'settings' && (s.update.phase === 'ready' || !s.firewallOk)}<span class="pip"></span>{/if}
        </button>
      {/each}
    </div>

    <div class="grow"></div>

    <div class="device" class:on>
      <Orb state={s.state} size={38} />
      <div class="grow">
        <div class="dn ellipsis">{on ? (s.peer?.name ?? t('app.phone')) : s.peers[0]?.name ?? t('app.noPhone')}</div>
        <div class="ds small">
          {on ? t('app.connected') : s.state === 'Connecting' ? t('app.connecting') : s.peers.length ? t('app.waiting') : t('app.unpaired')}
        </div>
      </div>
      {#if on && s.battery}
        <BatteryRing level={s.battery.level} charging={s.battery.charging} size={36} />
      {/if}
    </div>
  </nav>

  <section class="content">
    <Titlebar title={tabLabel(tab)} />
    <main bind:this={mainEl}>
      {#if !app.ready}
        <div class="loading" in:fade><Orb state="Connecting" size={44} /></div>
      {:else}
        {#key tab}
          <div class="view" in:fly={{ y: 10, duration: 260, opacity: 0 }}>
            {#if tab === 'home'}
              <Home go={(x: Tab) => (tab = x)} />
            {:else if tab === 'files'}
              <Files />
            {:else if tab === 'notifs'}
              <Notifs />
            {:else if tab === 'remote'}
              <Remote />
            {:else if tab === 'camera'}
              <Camera />
            {:else if tab === 'suite'}
              <Suite />
            {:else}
              <SettingsPage />
            {/if}
          </div>
        {/key}
      {/if}
    </main>
  </section>

  {#if dragging}
    <div class="drop" transition:fade={{ duration: 160 }}>
      <div class="dropbox" in:scale={{ start: 0.96, duration: 220 }}>
        <div class="dropicon"><Icon name="upload" size={34} /></div>
        <div class="dt">{t('app.dropTitle')}</div>
        <div class="muted">{s.peers.length ? (on ? t('app.dropReady', { name: s.peer?.name ?? '' }) : t('app.dropWait')) : t('app.needPhone')}</div>
      </div>
    </div>
  {/if}

  {#if crash}
    <ReportDialog preview={crash} manual={false} onclose={() => (crash = null)} />
  {/if}

  {#if app.toast}
    <div class="toastwrap">
      <div class="toast" transition:fly={{ y: 12, duration: 220 }}>
        <span class="tdot"></span>{app.toast}
      </div>
    </div>
  {/if}
</div>

<style>
  .shell {
    display: flex;
    height: 100%;
    background:
      radial-gradient(900px 420px at 85% -10%, rgba(227, 169, 43, 0.07), transparent 60%),
      radial-gradient(700px 500px at 100% 110%, rgba(139, 92, 246, 0.035), transparent 60%),
      var(--bg);
  }
  nav {
    width: 236px;
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 0 12px 12px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.015), transparent 40%), var(--bg-2);
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 18px 10px 22px;
  }
  .name {
    font-family: 'Segoe UI Variable Display', 'Segoe UI', sans-serif;
    font-size: 17px;
    font-weight: 680;
    letter-spacing: -0.01em;
    background: linear-gradient(90deg, var(--text), var(--gold-hi));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .ver {
    font-size: 11px;
    margin-top: -1px;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .navbtn {
    position: relative;
    border: 1px solid transparent;
    background: transparent;
    justify-content: flex-start;
    height: 40px;
    padding: 0 12px;
    width: 100%;
    color: var(--text-2);
    font-weight: 520;
    text-align: left;
  }
  .navbtn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.03);
    border-color: transparent;
    color: var(--text);
  }
  .navbtn .ico {
    display: flex;
    color: var(--muted);
    transition: color var(--t) var(--ease);
  }
  .navbtn.sel {
    background: linear-gradient(90deg, var(--gold-soft), rgba(227, 169, 43, 0.02));
    border-color: rgba(227, 169, 43, 0.14);
    color: var(--text);
  }
  .navbtn.sel .ico {
    color: var(--gold-hi);
  }
  .navbtn.sel::before {
    content: '';
    position: absolute;
    left: -12px;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--gold-grad);
    box-shadow: 0 0 12px rgba(245, 197, 66, 0.6);
  }
  .count {
    font-size: 11px;
    font-weight: 650;
    background: var(--gold-soft);
    color: var(--gold-hi);
    border-radius: 99px;
    padding: 1px 8px;
    min-width: 22px;
    text-align: center;
  }
  .count.violet {
    background: var(--violet-soft);
    color: #c4b1ff;
  }
  .pip {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--gold-hi);
    box-shadow: 0 0 8px var(--gold);
  }
  .device {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 8px;
    border-radius: var(--r);
    background: var(--s1);
    border: 1px solid var(--line);
    transition: border-color 400ms var(--ease);
  }
  .device.on {
    border-color: rgba(227, 169, 43, 0.18);
  }
  .dn {
    font-weight: 600;
    font-size: 13px;
  }
  .ds {
    color: var(--muted);
  }
  .device.on .ds {
    color: var(--gold-hi);
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 6px 32px 32px;
  }
  .loading {
    height: 60%;
    display: grid;
    place-items: center;
  }
  .drop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(5, 5, 6, 0.72);
    backdrop-filter: blur(6px);
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .dropbox {
    width: min(560px, 100%);
    height: min(340px, 80%);
    border-radius: 28px;
    border: 2px dashed var(--gold-line);
    background: radial-gradient(circle at 50% 35%, rgba(227, 169, 43, 0.12), transparent 65%);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    animation: glowpulse 1.8s ease-in-out infinite;
  }
  .dropicon {
    width: 72px;
    height: 72px;
    border-radius: 22px;
    display: grid;
    place-items: center;
    background: var(--gold-grad);
    color: var(--on-gold);
    box-shadow: 0 14px 40px -10px rgba(227, 169, 43, 0.7);
    margin-bottom: 8px;
    animation: bob 1.6s ease-in-out infinite;
  }
  .dt {
    font-size: 20px;
    font-weight: 650;
  }
  @keyframes glowpulse {
    50% {
      border-color: rgba(245, 197, 66, 0.65);
    }
  }
  @keyframes bob {
    50% {
      transform: translateY(-5px);
    }
  }
  .toastwrap {
    position: fixed;
    bottom: 22px;
    left: 236px;
    right: 0;
    display: flex;
    justify-content: center;
    pointer-events: none;
    z-index: 60;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--glass);
    backdrop-filter: blur(14px);
    border: 1px solid var(--line-2);
    color: var(--text);
    padding: 10px 16px;
    border-radius: 12px;
    font-size: 13px;
    box-shadow: var(--shadow-lg);
  }
  .tdot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--gold-hi);
    box-shadow: 0 0 8px var(--gold);
  }
</style>
