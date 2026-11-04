<script lang="ts">
  import { app, call, fmtTime } from './store.svelte'
  import Icon from './Icon.svelte'

  const s = $derived(app.s)
  let filter = $state('')

  const apps = $derived(
    [...new Map(s.notifications.map((n) => [n.package, n.app_name])).entries()].sort((a, b) => a[1].localeCompare(b[1], 'tr')),
  )
  const list = $derived(filter ? s.notifications.filter((n) => n.package === filter) : s.notifications)

  async function dismiss(key: string) {
    s.notifications = s.notifications.filter((n) => n.key !== key)
    await call('dismiss_notification', { key })
  }

  async function clearAll() {
    s.notifications = []
    await call('clear_notifications')
  }

  async function mute(pkg: string, name: string) {
    if (!confirm(`${name} bildirimleri artık gösterilmesin mi? (Ayarlar'dan geri açabilirsin)`)) return
    s.settings.mutedApps = [...s.settings.mutedApps, pkg]
    s.notifications = s.notifications.filter((n) => n.package !== pkg)
    if (filter === pkg) filter = ''
    await call('mute_app', { package: pkg, muted: true })
  }
</script>

<div class="page">
  <div class="row">
    <h2 class="grow">Telefon bildirimleri</h2>
    {#if s.notifications.length}
      <button class="ghost" onclick={clearAll}><Icon name="trash" size={16} /> Tümünü kapat</button>
    {/if}
  </div>

  {#if apps.length > 1}
    <div class="chips">
      <button class:sel={filter === ''} onclick={() => (filter = '')}>Tümü ({s.notifications.length})</button>
      {#each apps as [pkg, name] (pkg)}
        <button class:sel={filter === pkg} onclick={() => (filter = pkg)}>
          {#if s.icons[pkg]}<img src={s.icons[pkg]} alt="" />{/if}
          {name}
        </button>
      {/each}
    </div>
  {/if}

  {#if list.length === 0}
    <div class="empty muted">
      {s.state === 'Connected' ? 'Yeni bildirim yok.' : 'Telefon bağlanınca bildirimler burada görünür.'}
      <div class="small">Telefonda Köprü'ye “Bildirim erişimi” izni verilmiş olmalı.</div>
    </div>
  {:else}
    <div class="list">
      {#each list as n (n.key)}
        <div class="card item">
          <div class="appicon">
            {#if s.icons[n.package]}<img src={s.icons[n.package]} alt="" />{:else}<Icon name="bell" />{/if}
          </div>
          <div class="grow">
            <div class="row top">
              <span class="muted small grow ellipsis">{n.app_name}</span>
              <span class="muted small">{fmtTime(n.time)}</span>
            </div>
            {#if n.title}<div class="title selectable">{n.title}</div>{/if}
            {#if n.text}<div class="text selectable">{n.text}</div>{/if}
          </div>
          <div class="actions">
            <button class="ghost icon" title="Kapat (telefonda da)" onclick={() => dismiss(n.key)}><Icon name="x" size={16} /></button>
            <button class="ghost icon" title="Bu uygulamayı sustur" onclick={() => mute(n.package, n.app_name)}><Icon name="eyeoff" size={16} /></button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    display: grid;
    gap: 12px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chips button {
    border-radius: 99px;
    padding: 4px 10px;
    font-size: 12px;
  }
  .chips button.sel {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .chips img {
    width: 14px;
    height: 14px;
  }
  .empty {
    padding: 40px;
    text-align: center;
  }
  .small {
    font-size: 12px;
  }
  .list {
    display: grid;
    gap: 8px;
  }
  .item {
    display: flex;
    gap: 12px;
    padding: 12px 14px;
  }
  .appicon {
    width: 32px;
    height: 32px;
    flex: none;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .appicon img {
    width: 32px;
    height: 32px;
    border-radius: 8px;
  }
  .title {
    font-weight: 600;
  }
  .text {
    white-space: pre-wrap;
    word-break: break-word;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
</style>
