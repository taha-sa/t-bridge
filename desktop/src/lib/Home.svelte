<script lang="ts">
  import { app, call, flash } from './store.svelte'
  import type { Pairing } from './types'
  import Icon from './Icon.svelte'

  let pairing = $state<Pairing | null>(null)
  let showPair = $state(false)
  let timer: ReturnType<typeof setInterval> | undefined

  const s = $derived(app.s)
  const paired = $derived(s.peers.length > 0)

  async function openPairing() {
    showPair = true
    pairing = (await call<Pairing>('pairing')) ?? null
    clearInterval(timer)
    // Kod 5 dk geçerli; ekran açıkken 4 dk'da bir yenile.
    timer = setInterval(async () => {
      if (showPair) pairing = (await call<Pairing>('pairing')) ?? null
    }, 240_000)
  }

  function closePairing() {
    showPair = false
    clearInterval(timer)
  }

  $effect(() => {
    if (app.ready && !paired && !showPair) openPairing()
  })

  $effect(() => {
    // Eşleşme gerçekleşince QR ekranını kapat.
    if (showPair && s.state === 'Connected' && s.peer) closePairing()
  })

  $effect(() => () => clearInterval(timer))

  async function unpair(id: string, name: string) {
    if (!confirm(`${name} eşleşmesi kaldırılsın mı?`)) return
    await call('unpair', { id })
    s.peers = s.peers.filter((p) => p.id !== id)
    flash('Eşleşme kaldırıldı')
  }

  const stateText = $derived(
    s.state === 'Connected' ? `${s.peer?.name ?? 'Telefon'} bağlı` : s.state === 'Connecting' ? 'Bağlanıyor…' : paired ? 'Telefon bekleniyor' : 'Eşleşmiş telefon yok',
  )
</script>

<div class="page">
  <section class="card hero">
    <div class="dot" class:on={s.state === 'Connected'} class:mid={s.state === 'Connecting'}></div>
    <div class="grow">
      <h1>{stateText}</h1>
      <div class="muted">
        Bu bilgisayar: <b>{s.deviceName}</b>
        {#if s.port}· UDP {s.port}{/if}
      </div>
    </div>
    {#if s.state === 'Connected' && s.battery}
      <div class="battery" class:low={s.battery.level <= 15 && !s.battery.charging}>
        <Icon name={s.battery.charging ? 'bolt' : 'battery'} />
        <span>%{s.battery.level}</span>
      </div>
    {/if}
  </section>

  {#if s.error}
    <section class="card err">
      <Icon name="alert" />
      <div>
        <b>Çekirdek başlatılamadı.</b>
        <div class="selectable">{s.error}</div>
      </div>
    </section>
  {/if}

  {#if showPair}
    <section class="card pair">
      <div class="qr">
        {#if pairing}
          {@html pairing.svg}
        {:else}
          <div class="muted">Hazırlanıyor…</div>
        {/if}
      </div>
      <div class="grow steps">
        <h2>Telefonu eşleştir</h2>
        <ol>
          <li>Telefonda <b>Köprü</b> uygulamasını aç.</li>
          <li><b>“QR ile eşleştir”</b>e dokun ve bu kodu okut.</li>
          <li>Ya da listeden <b>{pairing?.name ?? s.deviceName}</b>’i seçip bu kodu gir:</li>
        </ol>
        <div class="code selectable">{pairing?.code.replace(/(\d{3})(\d{3})/, '$1 $2') ?? '··· ···'}</div>
        <div class="muted small">Kod 5 dakika geçerlidir. Telefon ve bilgisayar aynı Wi-Fi'da olmalı.</div>
        {#if pairing && pairing.addrs.length === 0}
          <div class="warnline"><Icon name="alert" size={16} /> Ağ bağlantısı bulunamadı.</div>
        {/if}
        {#if paired}
          <button class="ghost" onclick={closePairing}>Kapat</button>
        {/if}
      </div>
    </section>
  {:else}
    <section class="card">
      <div class="row">
        <h2 class="grow">Eşleşmiş cihazlar</h2>
        <button onclick={openPairing}><Icon name="link" size={16} /> Yeni telefon eşleştir</button>
      </div>
      {#each s.peers as p (p.id)}
        <div class="peer row">
          <Icon name="remote" />
          <div class="grow">
            <div><b>{p.name}</b> {#if s.peer?.id === p.id}<span class="badge">bağlı</span>{/if}</div>
            <div class="muted small ellipsis">Parmak izi {p.fingerprint.slice(0, 16)}…</div>
          </div>
          <button class="ghost" onclick={() => unpair(p.id, p.name)}>Kaldır</button>
        </div>
      {/each}
    </section>
  {/if}

  {#if s.state === 'Connected'}
    <section class="card tips">
      <div class="tip"><Icon name="clip" /> Kopyaladığın metin telefonda da kopyalanmış olur.</div>
      <div class="tip"><Icon name="files" /> Dosyaları pencereye sürükle-bırak ile gönder.</div>
      <div class="tip"><Icon name="ring" /> Telefonu bulmak için Kumanda → Çaldır.</div>
    </section>
  {/if}
</div>

<style>
  .page {
    display: grid;
    gap: 14px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
  }
  .dot.on {
    background: var(--ok);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--ok) 20%, transparent);
  }
  .dot.mid {
    background: var(--warn);
  }
  .battery {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 16px;
  }
  .battery.low {
    color: var(--bad);
  }
  .err {
    display: flex;
    gap: 10px;
    color: var(--bad);
  }
  .pair {
    display: flex;
    gap: 24px;
    align-items: center;
  }
  .qr {
    width: 240px;
    height: 240px;
    border-radius: 12px;
    overflow: hidden;
    background: #fff;
    display: grid;
    place-items: center;
    flex: none;
  }
  .qr :global(svg) {
    width: 240px;
    height: 240px;
  }
  .steps ol {
    padding-left: 18px;
    margin: 10px 0;
  }
  .code {
    font-size: 34px;
    font-weight: 700;
    letter-spacing: 4px;
    font-variant-numeric: tabular-nums;
    margin: 4px 0 6px;
  }
  .small {
    font-size: 12px;
  }
  .warnline {
    color: var(--warn);
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .peer {
    padding: 10px 0;
    border-top: 1px solid var(--line);
    margin-top: 10px;
  }
  .badge {
    font-size: 11px;
    background: color-mix(in srgb, var(--ok) 18%, transparent);
    color: var(--ok);
    padding: 1px 7px;
    border-radius: 99px;
    margin-left: 6px;
  }
  .tips {
    display: grid;
    gap: 8px;
  }
  .tip {
    display: flex;
    gap: 10px;
    align-items: center;
    color: var(--muted);
  }
</style>
