<script lang="ts">
  import { flip } from 'svelte/animate'
  import { fly } from 'svelte/transition'
  import { app, call, fmtTime } from './store.svelte'
  import type { NotificationData } from './types'
  import Icon from './Icon.svelte'

  const s = $derived(app.s)
  let filter = $state('')

  const apps = $derived(
    [...new Map(s.notifications.map((n) => [n.package, n.app_name])).entries()].sort((a, b) => a[1].localeCompare(b[1], 'tr')),
  )
  const count = (pkg: string) => s.notifications.filter((n) => n.package === pkg).length
  const list = $derived(filter ? s.notifications.filter((n) => n.package === filter) : s.notifications)

  // Ardışık aynı uygulamanın bildirimleri tek grupta: başlık bir kez yazılır.
  const groups = $derived.by(() => {
    const out: { pkg: string; name: string; items: NotificationData[] }[] = []
    for (const n of list) {
      const last = out[out.length - 1]
      if (last && last.pkg === n.package) last.items.push(n)
      else out.push({ pkg: n.package, name: n.app_name, items: [n] })
    }
    return out
  })

  async function dismiss(key: string) {
    s.notifications = s.notifications.filter((n) => n.key !== key)
    await call('dismiss_notification', { key })
  }

  async function dismissGroup(items: NotificationData[]) {
    const keys = new Set(items.map((n) => n.key))
    s.notifications = s.notifications.filter((n) => !keys.has(n.key))
    for (const k of keys) await call('dismiss_notification', { key: k })
  }

  async function clearAll() {
    s.notifications = []
    filter = ''
    await call('clear_notifications')
  }

  async function mute(pkg: string, name: string) {
    if (!confirm(`${name} bildirimleri artık gösterilmesin mi? (Ayarlar’dan geri açabilirsin)`)) return
    s.settings.mutedApps = [...s.settings.mutedApps, pkg]
    s.notifications = s.notifications.filter((n) => n.package !== pkg)
    if (filter === pkg) filter = ''
    await call('mute_app', { package: pkg, muted: true })
  }
</script>

<div class="page">
  <div class="row headrow">
    <div class="grow">
      <div class="muted small">
        {s.notifications.length ? `${s.notifications.length} bildirim · kapattığın telefondan da kalkar` : 'Telefon bildirimleri burada'}
      </div>
    </div>
    {#if s.notifications.length}
      <button class="ghost sm" onclick={clearAll}><Icon name="trash" size={15} /> Tümünü kapat</button>
    {/if}
  </div>

  {#if apps.length > 1}
    <div class="chips">
      <button class="pill" class:sel={filter === ''} onclick={() => (filter = '')}>
        Tümü <span class="num c">{s.notifications.length}</span>
      </button>
      {#each apps as [pkg, name] (pkg)}
        <button class="pill" class:sel={filter === pkg} onclick={() => (filter = pkg)}>
          {#if s.icons[pkg]}<img src={s.icons[pkg]} alt="" />{/if}
          {name} <span class="num c">{count(pkg)}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if list.length === 0}
    <section class="empty card">
      <div class="bellart">
        <Icon name="bell" size={30} />
        <span class="ping"></span>
      </div>
      <h2>{s.state === 'Connected' ? 'Her şey sakin' : 'Telefon bağlı değil'}</h2>
      <p class="muted">
        {s.state === 'Connected'
          ? 'Yeni bir bildirim gelince burada belirir.'
          : 'Bağlanınca telefondaki bildirimler burada görünür.'}
        <br /><span class="small faint">Telefonda T-Bridge’e “Bildirim erişimi” izni verilmiş olmalı.</span>
      </p>
    </section>
  {:else}
    <div class="groups">
      {#each groups as g, gi (g.pkg + gi)}
        <section class="group card" animate:flip={{ duration: 240 }} in:fly={{ y: 8, duration: 240 }}>
          <header class="row gh">
            <div class="appicon">
              {#if s.icons[g.pkg]}<img src={s.icons[g.pkg]} alt="" />{:else}<Icon name="bell" size={16} />{/if}
            </div>
            <span class="an grow ellipsis">{g.name}</span>
            <button class="ghost icon sm" title="Bu uygulamayı sustur" onclick={() => mute(g.pkg, g.name)}><Icon name="eyeoff" size={15} /></button>
            {#if g.items.length > 1}
              <button class="ghost sm" onclick={() => dismissGroup(g.items)}>Hepsini kapat</button>
            {/if}
          </header>
          {#each g.items as n (n.key)}
            <article class="item" animate:flip={{ duration: 200 }}>
              <div class="grow">
                <div class="row top">
                  {#if n.title}<span class="title grow ellipsis selectable">{n.title}</span>{:else}<span class="grow"></span>{/if}
                  <span class="faint small num">{fmtTime(n.time)}</span>
                </div>
                {#if n.text}<div class="text selectable">{n.text}</div>{/if}
              </div>
              <button class="ghost icon sm x" title="Kapat (telefonda da)" onclick={() => dismiss(n.key)}><Icon name="x" size={15} /></button>
            </article>
          {/each}
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .headrow {
    min-height: 30px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pill {
    height: 32px;
    border-radius: 99px;
    padding: 0 12px 0 10px;
    font-size: 13px;
    background: var(--s2);
    border-color: var(--line);
  }
  .pill img {
    width: 18px;
    height: 18px;
    border-radius: 5px;
  }
  .pill .c {
    font-size: 11px;
    color: var(--muted);
  }
  .pill.sel {
    background: var(--gold-soft);
    border-color: var(--gold-line);
    color: var(--gold-hi);
  }
  .pill.sel .c {
    color: var(--gold);
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .group {
    padding: 6px 8px 8px;
  }
  .gh {
    gap: 10px;
    padding: 8px 8px 6px;
  }
  .appicon {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    flex: none;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--s3);
    color: var(--muted);
  }
  .appicon img {
    width: 100%;
    height: 100%;
  }
  .an {
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .item {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 8px 10px 44px;
    border-radius: 12px;
    transition: background var(--t) var(--ease);
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.025);
  }
  .item + .item {
    border-top: 1px solid var(--line);
  }
  .item .x {
    opacity: 0;
    transition: opacity var(--t) var(--ease);
  }
  .item:hover .x {
    opacity: 1;
  }
  .top {
    gap: 10px;
  }
  .title {
    font-weight: 620;
  }
  .text {
    margin-top: 2px;
    color: var(--text-2);
    white-space: pre-wrap;
    word-break: break-word;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 48px 20px;
  }
  .empty p {
    margin: 6px 0 0;
  }
  .bellart {
    position: relative;
    width: 68px;
    height: 68px;
    border-radius: 22px;
    display: grid;
    place-items: center;
    margin-bottom: 16px;
    color: #c4b1ff;
    background: var(--violet-soft);
    border: 1px solid rgba(139, 92, 246, 0.3);
  }
  .ping {
    position: absolute;
    top: 14px;
    right: 16px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--violet);
    box-shadow: 0 0 0 0 rgba(139, 92, 246, 0.6);
    animation: ping 2.4s ease-out infinite;
  }
  @keyframes ping {
    0% {
      box-shadow: 0 0 0 0 rgba(139, 92, 246, 0.6);
    }
    70%,
    100% {
      box-shadow: 0 0 0 12px rgba(139, 92, 246, 0);
    }
  }
</style>
