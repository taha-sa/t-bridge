<script lang="ts">
  import { app, call, flash, fmtBytes } from './store.svelte'
  import type { TransferInfo } from './types'
  import Icon from './Icon.svelte'

  let { dragging }: { dragging: boolean } = $props()

  const s = $derived(app.s)
  const order: Record<string, number> = { Active: 0, Queued: 1, Paused: 2, Failed: 3, Done: 4, Cancelled: 5 }
  const list = $derived([...s.transfers].sort((a, b) => order[a.state] - order[b.state] || b.id - a.id))
  const finished = $derived(s.transfers.some((t) => ['Done', 'Failed', 'Cancelled'].includes(t.state)))

  const stateLabel: Record<string, string> = {
    Queued: 'Sırada',
    Active: 'Aktarılıyor',
    Paused: 'Duraklatıldı',
    Done: 'Tamamlandı',
    Failed: 'Başarısız',
    Cancelled: 'İptal edildi',
  }

  function pct(t: TransferInfo) {
    return t.size === 0 ? (t.state === 'Done' ? 100 : 0) : Math.min(100, (t.done / t.size) * 100)
  }

  function eta(t: TransferInfo) {
    if (t.state !== 'Active' || t.speed <= 0) return ''
    const sec = Math.round((t.size - t.done) / t.speed)
    if (sec < 60) return `${sec} sn`
    if (sec < 3600) return `${Math.round(sec / 60)} dk`
    return `${(sec / 3600).toFixed(1)} sa`
  }

  async function pick(folder: boolean) {
    const n = await call<number>('pick_and_send', { folder })
    if (n) flash(`${n} dosya sıraya eklendi`)
  }

  const args = (t: TransferInfo) => ({ id: t.id, direction: t.direction })
  const shortName = (n: string) => n.split('/').pop() ?? n
  const dirName = (n: string) => (n.includes('/') ? n.slice(0, n.lastIndexOf('/')) : '')
</script>

<div class="page">
  <section class="drop" class:active={dragging}>
    <Icon name="up" size={28} />
    <div>
      <b>Dosya veya klasörleri buraya sürükle</b>
      <div class="muted">
        {s.state === 'Connected' ? 'Telefona hemen gönderilir.' : 'Telefon bağlanınca otomatik gönderilir (kuyruk).'}
      </div>
    </div>
    <div class="grow"></div>
    <button onclick={() => pick(false)}>Dosya seç</button>
    <button onclick={() => pick(true)}>Klasör seç</button>
  </section>

  <div class="row head">
    <h2 class="grow">Aktarımlar</h2>
    <button class="ghost" onclick={() => call('open_downloads')}><Icon name="folder" size={16} /> Gelenler klasörü</button>
    {#if finished}
      <button class="ghost" onclick={() => call('clear_finished')}>Bitenleri temizle</button>
    {/if}
  </div>

  {#if list.length === 0}
    <div class="empty muted">Henüz aktarım yok. Telefondan gönderilen dosyalar <span class="selectable">{s.downloadDir}</span> klasörüne kaydedilir.</div>
  {:else}
    <div class="list">
      {#each list as t (t.direction + t.id)}
        <div class="item card" class:done={t.state === 'Done'} class:bad={t.state === 'Failed'}>
          <div class="dir" class:in={t.direction === 'Incoming'}><Icon name={t.direction === 'Incoming' ? 'down' : 'up'} size={16} /></div>
          <div class="grow">
            <div class="row">
              <div class="grow ellipsis" title={t.name}>
                <b>{shortName(t.name)}</b>
                {#if dirName(t.name)}<span class="muted"> · {dirName(t.name)}</span>{/if}
              </div>
              <div class="muted nums">
                {#if t.state === 'Active'}{fmtBytes(t.speed)}/s · {eta(t)} ·{/if}
                {fmtBytes(t.done)} / {fmtBytes(t.size)}
              </div>
            </div>
            <div class="bar"><div style:width="{pct(t)}%" class:paused={t.state !== 'Active'}></div></div>
            <div class="row meta">
              <span class="muted grow ellipsis">
                {stateLabel[t.state]}{#if t.state === 'Failed' && t.detail}: {t.detail}{/if}
                {#if t.state === 'Queued' && s.state !== 'Connected'} — bağlantı bekleniyor{/if}
              </span>
              {#if t.state === 'Active' || t.state === 'Queued'}
                <button class="ghost icon" title="Duraklat" onclick={() => call('pause_transfer', args(t))}><Icon name="pause" size={16} /></button>
              {/if}
              {#if t.state === 'Paused'}
                <button class="ghost icon" title="Devam et" onclick={() => call('resume_transfer', args(t))}><Icon name="play" size={16} /></button>
              {/if}
              {#if ['Active', 'Queued', 'Paused'].includes(t.state)}
                <button class="ghost icon" title="İptal" onclick={() => call('cancel_transfer', args(t))}><Icon name="x" size={16} /></button>
              {/if}
              {#if t.state === 'Done' && t.direction === 'Incoming' && t.detail}
                <button class="ghost" onclick={() => call('reveal', { path: t.detail })}><Icon name="folder" size={16} /> Göster</button>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    display: grid;
    gap: 14px;
  }
  .drop {
    display: flex;
    align-items: center;
    gap: 14px;
    border: 2px dashed var(--line);
    border-radius: var(--radius);
    padding: 20px;
    background: var(--panel);
    transition: 0.15s;
  }
  .drop.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .head {
    margin-top: 4px;
  }
  .empty {
    padding: 30px;
    text-align: center;
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
  .dir {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }
  .dir.in {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .nums {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
    white-space: nowrap;
  }
  .bar {
    height: 6px;
    background: var(--panel-2);
    border-radius: 99px;
    overflow: hidden;
    margin: 8px 0 4px;
  }
  .bar > div {
    height: 100%;
    background: var(--accent);
    border-radius: 99px;
    transition: width 0.25s linear;
  }
  .bar > div.paused {
    background: var(--muted);
  }
  .item.done .bar > div {
    background: var(--ok);
  }
  .item.bad .bar > div {
    background: var(--bad);
  }
  .meta {
    gap: 4px;
    font-size: 12px;
  }
</style>
