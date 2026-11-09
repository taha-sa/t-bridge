<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { app, flash } from './store.svelte'
  import Icon from './Icon.svelte'

  interface PhoneCam {
    active: boolean
    w: number
    h: number
    fps: number
    facing: number
    mirror: boolean
  }
  interface CameraStatus {
    supported: boolean
    installed: boolean
    running: boolean
    phoneSupports: boolean
    phone: PhoneCam
    inUse: boolean
    decodedFrames: number
  }

  let st = $state<CameraStatus | null>(null)
  let busy = $state(false)
  let error = $state('')
  let loadFailed = $state(false)
  let retrying = $state(false)
  const on = $derived(app.s.state === 'Connected')

  async function refresh() {
    try {
      st = await invoke<CameraStatus>('camera_status')
      loadFailed = false
    } catch {
      // Durum hiç okunamadıysa (st hâlâ null) iskelet yerine hata + yeniden dene gösterilir.
      if (st === null) loadFailed = true
    }
  }

  async function retry() {
    retrying = true
    await refresh()
    retrying = false
  }

  async function run(cmd: string) {
    busy = true
    error = ''
    try {
      await invoke(cmd)
      flash(cmd === 'camera_enable' ? 'Kamera etkin' : cmd === 'camera_disable' ? 'Kamera durduruldu' : 'Kamera kaldırıldı')
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
      refresh()
    }
  }

  onMount(() => {
    refresh()
    const t = setInterval(refresh, 2000)
    return () => clearInterval(t)
  })

  const label = $derived.by(() => {
    if (!st || !st.running) return 'Etkin değil'
    if (st.phone.active) return `Yayında · ${st.phone.w}×${st.phone.h} · ${st.phone.fps} fps`
    return 'Hazır'
  })
  const tone = $derived(st?.running ? (st.phone.active ? 'live' : 'ready') : 'off')
</script>

<div class="page">
  {#if st === null && loadFailed}
    <section class="card loadfail" role="alert">
      <div class="lf-title">Kamera durumu okunamadı</div>
      <div class="muted small">Arka uç yanıt vermiyor olabilir. Biraz sonra yeniden deneyin.</div>
      <button class="sm" disabled={retrying} onclick={retry}>{retrying ? 'Deneniyor…' : 'Yeniden dene'}</button>
    </section>
  {:else if st === null}
    <div class="skeleton" aria-busy="true"></div>
  {:else if !st.supported}
    <!-- Windows 10 ve altı: kilitli durum, düğme yok -->
    <section class="card locked" aria-live="polite">
      <div class="lockart"><Icon name="lock" size={28} /></div>
      <h2>Telefon kamerası</h2>
      <p class="dim">Bu özellik yalnızca Windows 11 için tasarlanmıştır.</p>
    </section>
  {:else}
    <section class="card cam">
      <header class="row">
        <div class="camicon"><Icon name="video" size={22} /></div>
        <div class="grow">
          <h2>Telefon kamerası</h2>
          <div class="muted small">Telefonu bu bilgisayarda web kamerası olarak kullan (Zoom, Teams, Kamera uygulaması…)</div>
        </div>
        <span class="status {tone}"><span class="dot"></span>{label}</span>
      </header>

      <div class="actions">
        {#if !st?.running}
          <button class="primary" disabled={busy || !st} onclick={() => run('camera_enable')}>
            {busy ? 'Hazırlanıyor…' : 'Kamerayı etkinleştir'}
          </button>
        {:else}
          <button disabled={busy} onclick={() => run('camera_disable')}>Durdur</button>
        {/if}
        {#if st?.installed}
          <button class="ghost" disabled={busy} onclick={() => run('camera_uninstall')}>Kamerayı kaldır</button>
        {/if}
      </div>
      {#if !st?.installed}
        <p class="hint small"><Icon name="info" size={14} /> Bir kez yönetici izni gerekir (kamera sürücüsüz bir Windows bileşeni olarak kaydedilir).</p>
      {/if}
      {#if error}<p class="err small" role="alert">{error}</p>{/if}

      <ul class="steps small muted">
        <li class:ok={st?.running}>1. “Kamerayı etkinleştir” — Windows’ta “T-Bridge Kamera” görünür.</li>
        <li class:ok={on && st?.phoneSupports}>2. Telefonda T-Bridge → Kumanda → Kamera bölümünden yayını başlat{on ? '' : ' (telefon bağlı değil)'}.</li>
        <li class:ok={st?.inUse}>3. Uygulamanda kamera olarak “T-Bridge Kamera”yı seç.</li>
      </ul>
    </section>

    <section class="card tight help">
      <h3>Sorun giderme</h3>
      <ul class="small muted">
        <li>Kamera listede görünmüyorsa: T-Bridge’i kapatıp açın, kamerayı kullanan uygulamayı (Zoom/Teams) yeniden başlatın.</li>
        <li>Görüntüde “Telefon bekleniyor” yazıyorsa telefondaki yayın kapalı ya da bağlantı kopmuş demektir.</li>
        <li>Telefonun ve bilgisayarın aynı Wi-Fi ağında olması en iyi gecikmeyi verir.</li>
        {#if st?.running && !st.phoneSupports && on}
          <li>Telefondaki T-Bridge sürümü kamerayı desteklemiyor; telefon uygulamasını güncelleyin.</li>
        {/if}
      </ul>
    </section>
  {/if}
</div>

<style>
  .cam {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .camicon {
    width: 44px;
    height: 44px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 12px;
    border-radius: 99px;
    font-size: 13px;
    font-weight: 600;
    background: var(--s2);
    border: 1px solid var(--line);
    color: var(--muted);
    transition:
      color 150ms var(--ease),
      border-color 150ms var(--ease),
      background 150ms var(--ease);
  }
  .status .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .status.ready {
    color: var(--gold-hi);
    border-color: var(--gold-line);
  }
  .status.ready .dot {
    background: var(--gold);
  }
  .status.live {
    color: var(--ok);
    border-color: var(--ok-soft);
    background: var(--ok-soft);
  }
  .status.live .dot {
    background: var(--ok);
    animation: beat 1.8s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      transform: scale(1.5);
      opacity: 0.6;
    }
  }
  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--muted);
  }
  .err {
    margin: 0;
    color: var(--bad);
  }
  .steps {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .steps li.ok {
    color: var(--gold-hi);
  }
  .help ul {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  /* Yükleniyor (durum henüz bilinmiyor): titremeyi önlemek için sabit yükseklikli iskelet */
  .skeleton {
    height: 180px;
    border-radius: 16px;
    background: var(--s1);
  }
  .loadfail {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 20px;
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .lf-title {
    font-weight: 600;
    color: var(--bad);
  }
  /* Windows 10: kilitli kart */
  .locked {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 48px 20px;
    gap: 6px;
  }
  .lockart {
    width: 64px;
    height: 64px;
    border-radius: 20px;
    display: grid;
    place-items: center;
    margin-bottom: 8px;
    color: var(--faint);
    background: var(--s2);
    border: 1px solid var(--line);
  }
  .dim {
    margin: 0;
    color: var(--faint);
  }
  @media (prefers-reduced-motion: reduce) {
    .status,
    .status .dot {
      transition: none;
      animation: none;
    }
  }
</style>
