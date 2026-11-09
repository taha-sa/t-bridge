<script lang="ts">
  import { fly } from 'svelte/transition'
  import { app, call, flash } from './store.svelte'
  import type { MediaAction, Pairing, PhoneCommand } from './types'
  import Icon from './Icon.svelte'
  import Orb from './ui/Orb.svelte'
  import BatteryRing from './ui/BatteryRing.svelte'

  let { go }: { go: (t: 'home' | 'files' | 'notifs' | 'remote' | 'settings') => void } = $props()

  const TTL = 300
  let pairing = $state<Pairing | null>(null)
  let issued = $state(0)
  let now = $state(Date.now())
  let showPair = $state(false)
  let ticker: ReturnType<typeof setInterval> | undefined

  const s = $derived(app.s)
  const paired = $derived(s.peers.length > 0)
  const on = $derived(s.state === 'Connected')
  const left = $derived(Math.max(0, TTL - Math.floor((now - issued) / 1000)))
  const digits = $derived((pairing?.code ?? '······').split(''))

  async function refreshCode() {
    pairing = (await call<Pairing>('pairing')) ?? null
    issued = Date.now()
  }

  async function openPairing() {
    showPair = true
    await refreshCode()
    clearInterval(ticker)
    ticker = setInterval(() => {
      now = Date.now()
      // Süre bitmeden yenile: ekrandaki kod her zaman geçerli olsun.
      if (showPair && left <= 20) refreshCode()
    }, 1000)
  }

  function closePairing() {
    showPair = false
    clearInterval(ticker)
  }

  $effect(() => {
    if (app.codeRev > 0 && showPair) refreshCode()
  })
  $effect(() => {
    if (app.ready && !paired && !showPair) openPairing()
  })
  $effect(() => {
    if (showPair && on && s.peer) closePairing()
  })
  $effect(() => () => clearInterval(ticker))

  async function unpair(id: string, name: string) {
    if (!confirm(`${name} eşleşmesi kaldırılsın mı?`)) return
    await call('unpair', { id })
    s.peers = s.peers.filter((p) => p.id !== id)
    flash('Eşleşme kaldırıldı')
  }

  async function sendFiles() {
    const n = await call<number>('pick_and_send', { folder: false })
    if (n) {
      flash(`${n} dosya sıraya eklendi`)
      go('files')
    }
  }
  const phone = (cmd: PhoneCommand) => call<boolean>('phone_cmd', { cmd })
  const media = (action: MediaAction) => phone({ Media: { action } })
  async function ring() {
    await phone({ Ring: { on: true } })
    flash('Telefon çalıyor (30 sn)')
  }
  async function fixFirewall() {
    const ok = await call<boolean>('fix_firewall')
    s.firewallOk = !!ok
    flash(ok ? 'Güvenlik duvarı izni verildi' : 'İzin verilmedi')
  }

  const title = $derived(
    on ? (s.peer?.name ?? 'Telefon') : s.state === 'Connecting' ? 'Bağlanıyor…' : paired ? 'Telefon bekleniyor' : 'Hoş geldin',
  )
  const subtitle = $derived(
    on
      ? 'Bağlı · pano, dosya ve bildirimler eşitleniyor'
      : paired
        ? 'Telefonda T-Bridge açıkken ve aynı Wi‑Fi’dayken kendiliğinden bağlanır.'
        : 'Başlamak için telefonunu bir kez eşleştir.',
  )
  const mmss = (n: number) => `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`
</script>

<div class="page">
  <section class="card hero" class:gold={on}>
    <div class="glow" class:on></div>
    <Orb state={s.state} size={78} />
    <div class="grow">
      <div class="eyebrow">{on ? 'Bağlı cihaz' : 'Durum'}</div>
      <h1 class="ellipsis">{title}</h1>
      <p class="muted">{subtitle}</p>
    </div>
    {#if on && s.battery}
      <div class="bat">
        <BatteryRing level={s.battery.level} charging={s.battery.charging} size={64} />
        <span class="small muted">{s.battery.charging ? 'Şarj oluyor' : 'Pil'}</span>
      </div>
    {/if}
  </section>

  {#if on}
    <div class="actions rise">
      <button class="primary" onclick={sendFiles}><Icon name="upload" size={17} /> Dosya gönder</button>
      <button onclick={ring}><Icon name="ring" size={17} /> Telefonu çaldır</button>
      <button onclick={() => go('notifs')}>
        <Icon name="bell" size={17} /> Bildirimler
        {#if s.notifications.length}<span class="chip violet num">{s.notifications.length}</span>{/if}
      </button>
      <button onclick={() => go('remote')}><Icon name="remote" size={17} /> Kumanda</button>
    </div>
  {/if}

  {#if !s.firewallOk}
    <section class="card tight warn rise">
      <Icon name="shield" />
      <div class="grow">
        <b>Güvenlik duvarı izni yok</b>
        <div class="muted small">Telefonun bu bilgisayara ulaşabilmesi için Windows’un bir kez izin vermesi gerekiyor.</div>
      </div>
      <button class="primary sm" onclick={fixFirewall}>İzin ver</button>
    </section>
  {/if}

  {#if s.error}
    <section class="card tight err">
      <Icon name="alert" />
      <div class="grow">
        <b>Çekirdek başlatılamadı</b>
        <div class="selectable small">{s.error}</div>
      </div>
    </section>
  {/if}

  {#if showPair}
    <section class="card gold pair" in:fly={{ y: 12, duration: 300 }}>
      <div class="qrwrap">
        <div class="qr">
          {#if pairing}
            {@html pairing.svg}
          {:else}
            <div class="qrph"></div>
          {/if}
        </div>
        <div class="corner tl"></div>
        <div class="corner tr"></div>
        <div class="corner bl"></div>
        <div class="corner br"></div>
      </div>
      <div class="grow steps">
        <div class="eyebrow">Eşleştirme</div>
        <h2>Telefonu bu bilgisayara bağla</h2>
        <ol>
          <li><span class="n">1</span>Telefonda <b>T-Bridge</b>’i aç.</li>
          <li><span class="n">2</span><b>QR ile eşleştir</b>’e dokunup bu kodu okut.</li>
          <li><span class="n">3</span>Ya da listeden <b>{pairing?.name ?? s.deviceName}</b>’i seçip kodu gir:</li>
        </ol>
        <div class="code selectable num" aria-label="Eşleştirme kodu">
          {#each digits as d, i (i)}
            <span class="dg" class:gap={i === 3}>{d}</span>
          {/each}
        </div>
        <div class="row timer">
          <div class="tbar"><div style="width:{(left / TTL) * 100}%"></div></div>
          <span class="small muted num">{mmss(left)}</span>
          <button class="ghost sm" onclick={refreshCode}><Icon name="refresh" size={14} /> Yenile</button>
        </div>
        {#if pairing && pairing.addrs.length === 0}
          <div class="warnline small"><Icon name="alert" size={15} /> Ağ bağlantısı bulunamadı — Wi‑Fi’ye bağlı mısın?</div>
        {/if}
        {#if paired}
          <button class="ghost sm close" onclick={closePairing}>Vazgeç</button>
        {/if}
      </div>
    </section>
  {/if}

  {#if on}
    <div class="grid">
      <section class="card np rise">
        <h3>Şimdi çalıyor</h3>
        {#if s.nowPlaying}
          <div class="row track">
            <div class="art" class:playing={s.nowPlaying.playing}>
              <Icon name="music" size={20} />
              <span class="eq"><i></i><i></i><i></i></span>
            </div>
            <div class="grow">
              <div class="ellipsis tt">{s.nowPlaying.title}</div>
              <div class="ellipsis small muted">{s.nowPlaying.artist || s.nowPlaying.app_name}</div>
            </div>
          </div>
          <div class="row mc">
            <button class="ghost icon" aria-label="Önceki" onclick={() => media('Previous')}><Icon name="prev" size={17} /></button>
            <button class="primary icon round" aria-label="Oynat/Duraklat" onclick={() => media('PlayPause')}>
              <Icon name={s.nowPlaying.playing ? 'pause' : 'play'} size={17} />
            </button>
            <button class="ghost icon" aria-label="Sonraki" onclick={() => media('Next')}><Icon name="next" size={17} /></button>
          </div>
        {:else}
          <div class="emptyline muted small">Telefonda müzik veya video çalınca burada görünür.</div>
        {/if}
      </section>

      <section class="card clip rise">
        <h3>Son gelen pano</h3>
        {#if app.lastClip}
          <div class="cliptext selectable">{app.lastClip}</div>
        {:else}
          <div class="emptyline muted small">
            Telefonda kopyaladığın metni bildirimdeki <b>“Panoyu PC’ye gönder”</b> ile yolla; burada ve panonda belirir.
          </div>
        {/if}
      </section>
    </div>
  {/if}

  {#if !on}
    <details class="card tight help rise" open={!s.firewallOk}>
      <summary><Icon name="info" size={18} /> Telefon bağlanamıyor mu?</summary>
      <ol>
        <li><span class="n">1</span><span>Telefonda T-Bridge açık mı, iki cihaz da aynı Wi‑Fi’da mı? Misafir ağları cihazların birbirini görmesini engeller.</span></li>
        <li><span class="n">2</span>
          <span>
            Güvenlik duvarı izni:
            {#if s.firewallOk}<b>verilmiş</b>.{:else}<b>yok</b>. Yukarıdaki <b>İzin ver</b>’e bas.{/if}
          </span>
        </li>
        <li><span class="n">3</span><span>Telefonda Wi‑Fi’ı kapatıp aç; VPN varsa kapat.</span></li>
        <li><span class="n">4</span><span>Hâlâ olmuyorsa modemde “istemci izolasyonu / AP izolasyonu” kapalı olmalı.</span></li>
      </ol>
    </details>
  {/if}

  {#if !showPair}
    <section class="card rise">
      <div class="row">
        <h3 class="grow">Eşleşmiş cihazlar</h3>
        <button class="sm" onclick={openPairing}><Icon name="link" size={15} /> Yeni telefon</button>
      </div>
      <div class="peers">
        {#each s.peers as p (p.id)}
          <div class="peer row">
            <div class="pico" class:live={s.peer?.id === p.id}><Icon name="phone" size={18} /></div>
            <div class="grow">
              <div class="row pn">
                <b class="ellipsis">{p.name}</b>
                {#if s.peer?.id === p.id}<span class="chip gold">bağlı</span>{/if}
              </div>
              <div class="faint small">{s.peer?.id === p.id ? 'Şifreli bağlantı' : 'Eşleşti'}</div>
            </div>
            <button class="ghost sm" onclick={() => unpair(p.id, p.name)}>Kaldır</button>
          </div>
        {:else}
          <div class="emptyline muted small">Henüz eşleşmiş telefon yok.</div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .eyebrow {
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--gold);
    margin-bottom: 4px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 26px 28px;
    overflow: hidden;
  }
  .hero h1 {
    font-size: 28px;
  }
  .hero p {
    margin: 4px 0 0;
  }
  .glow {
    position: absolute;
    inset: -40% -10% auto auto;
    width: 380px;
    height: 380px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(143, 140, 132, 0.06), transparent 65%);
    pointer-events: none;
    transition: background 600ms var(--ease);
  }
  .glow.on {
    background: radial-gradient(circle, rgba(245, 197, 66, 0.14), transparent 65%);
  }
  .bat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .actions button {
    height: 40px;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 14px;
    border-color: var(--gold-line);
    color: var(--text);
  }
  .warn :global(svg) {
    color: var(--gold-hi);
    flex: none;
  }
  .err {
    display: flex;
    gap: 14px;
    border-color: rgba(248, 113, 113, 0.3);
    color: var(--bad);
  }
  .pair {
    display: flex;
    gap: 32px;
    align-items: center;
    padding: 28px;
  }
  .qrwrap {
    position: relative;
    padding: 12px;
    flex: none;
  }
  .qr {
    width: 236px;
    height: 236px;
    background: #fff;
    border-radius: 18px;
    padding: 8px;
    display: grid;
    place-items: center;
    box-shadow: 0 20px 50px -18px rgba(227, 169, 43, 0.55);
  }
  .qr :global(svg) {
    width: 100%;
    height: 100%;
  }
  .qrph {
    width: 70%;
    height: 70%;
    border-radius: 10px;
    background: linear-gradient(90deg, #eee, #f8f8f8, #eee);
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .corner {
    position: absolute;
    width: 22px;
    height: 22px;
    border: 2.5px solid var(--gold);
  }
  .tl {
    top: 0;
    left: 0;
    border-right: none;
    border-bottom: none;
    border-radius: 10px 0 0 0;
  }
  .tr {
    top: 0;
    right: 0;
    border-left: none;
    border-bottom: none;
    border-radius: 0 10px 0 0;
  }
  .bl {
    bottom: 0;
    left: 0;
    border-right: none;
    border-top: none;
    border-radius: 0 0 0 10px;
  }
  .br {
    bottom: 0;
    right: 0;
    border-left: none;
    border-top: none;
    border-radius: 0 0 10px 0;
  }
  .steps h2 {
    font-size: 21px;
    margin-bottom: 14px;
  }
  ol {
    list-style: none;
    margin: 0 0 18px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 9px;
    color: var(--text-2);
  }
  .n {
    display: inline-grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin-right: 10px;
    border-radius: 50%;
    font-size: 12px;
    font-weight: 700;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  .code {
    display: flex;
    gap: 8px;
    margin-bottom: 14px;
  }
  .dg {
    width: 46px;
    height: 58px;
    display: grid;
    place-items: center;
    font-size: 30px;
    font-weight: 650;
    border-radius: 12px;
    background: var(--s3);
    border: 1px solid var(--line-2);
    color: var(--text);
  }
  .dg.gap {
    margin-left: 12px;
  }
  .timer {
    gap: 10px;
  }
  .tbar {
    flex: 1;
    max-width: 260px;
    height: 4px;
    border-radius: 99px;
    background: var(--s4);
    overflow: hidden;
  }
  .tbar div {
    height: 100%;
    background: var(--gold-grad);
    transition: width 1s linear;
  }
  .warnline {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 10px;
    color: var(--gold-hi);
  }
  .close {
    margin-top: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .grid h3 {
    margin-bottom: 14px;
  }
  .track {
    gap: 14px;
  }
  .art {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    flex: none;
    display: grid;
    place-items: center;
    position: relative;
    background: linear-gradient(135deg, rgba(139, 92, 246, 0.25), rgba(227, 169, 43, 0.18));
    color: var(--text);
    border: 1px solid var(--line-2);
  }
  .eq {
    position: absolute;
    bottom: 6px;
    right: 6px;
    display: none;
    gap: 2px;
    align-items: flex-end;
    height: 10px;
  }
  .playing .eq {
    display: flex;
  }
  .eq i {
    width: 2px;
    background: var(--gold-hi);
    border-radius: 1px;
    animation: eq 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(2) {
    animation-delay: 0.2s;
  }
  .eq i:nth-child(3) {
    animation-delay: 0.4s;
  }
  @keyframes eq {
    0%,
    100% {
      height: 3px;
    }
    50% {
      height: 10px;
    }
  }
  .tt {
    font-weight: 600;
  }
  .mc {
    justify-content: center;
    gap: 14px;
    margin-top: 16px;
  }
  .round {
    border-radius: 50%;
    width: 42px;
    height: 42px;
  }
  .cliptext {
    max-height: 96px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 13px;
    color: var(--text-2);
    padding: 12px 14px;
    border-radius: 12px;
    background: var(--s3);
    border: 1px solid var(--line);
  }
  .emptyline {
    padding: 6px 0;
  }
  .peers {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 12px;
  }
  .peer {
    padding: 10px;
    margin: 0 -10px;
    border-radius: 12px;
    transition: background var(--t) var(--ease);
  }
  .peer:hover {
    background: rgba(255, 255, 255, 0.025);
  }
  .pico {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    background: var(--s3);
    color: var(--muted);
    border: 1px solid var(--line);
  }
  .pico.live {
    color: var(--gold-hi);
    border-color: var(--gold-line);
    background: var(--gold-soft);
  }
  .pn {
    gap: 8px;
  }
  .help summary {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    list-style: none;
    font-weight: 600;
  }
  .help summary::-webkit-details-marker {
    display: none;
  }
  .help summary :global(svg) {
    color: var(--gold-hi);
    flex: none;
  }
  .help ol {
    margin: 12px 0 2px;
  }
  .help li {
    display: flex;
    gap: 2px;
    align-items: flex-start;
  }
  .help li .n {
    flex: none;
  }
</style>
