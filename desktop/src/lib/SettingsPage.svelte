<script lang="ts">
  import { app, call, flash } from './store.svelte'
  import type { Settings } from './types'

  const s = $derived(app.s)
  let restartNeeded = $state(false)

  async function save(patch: Partial<Settings>) {
    const next = { ...s.settings, ...patch }
    s.settings = next
    const r = await call<boolean>('set_settings', { settings: next })
    if (r) restartNeeded = true
  }

  async function pickDir() {
    const d = await call<string | null>('pick_download_dir')
    if (d) {
      await save({ downloadDir: d })
      s.downloadDir = d
    }
  }

  async function unmute(pkg: string) {
    s.settings.mutedApps = s.settings.mutedApps.filter((p) => p !== pkg)
    await call('mute_app', { package: pkg, muted: false })
    flash('Bildirimler tekrar açıldı')
  }

  const toggles: { key: keyof Settings; label: string; hint: string }[] = [
    { key: 'clipboardSync', label: 'Pano senkronu', hint: 'Kopyalanan metin iki cihazda da kopyalanmış olur.' },
    { key: 'notifToast', label: 'Bildirimleri Windows’ta göster', hint: 'Pencere kapalıyken telefon bildirimleri sağ altta çıkar.' },
    { key: 'transferToast', label: 'Dosya alınınca haber ver', hint: '' },
    { key: 'lowBatteryWarn', label: 'Telefonun şarjı azalınca uyar', hint: '%15 ve altında.' },
    { key: 'autostart', label: 'Windows açılışında başlat', hint: 'Tepside sessizce çalışır.' },
  ]
</script>

<div class="page">
  <section class="card">
    <h2>Genel</h2>
    {#each toggles as t (t.key)}
      <label class="toggle">
        <div class="grow">
          <div>{t.label}</div>
          {#if t.hint}<div class="muted small">{t.hint}</div>{/if}
        </div>
        <input type="checkbox" checked={s.settings[t.key] as boolean} onchange={(e) => save({ [t.key]: e.currentTarget.checked })} />
      </label>
    {/each}
  </section>

  <section class="card">
    <h2>Gelen dosyalar</h2>
    <div class="row dir">
      <div class="grow ellipsis selectable" title={s.downloadDir}>{s.downloadDir}</div>
      <button onclick={() => call('open_downloads')}>Aç</button>
      <button onclick={pickDir}>Değiştir…</button>
    </div>
    {#if restartNeeded}
      <div class="row warn">
        <span class="grow">Yeni klasör, uygulama yeniden başlayınca kullanılır.</span>
        <button class="primary" onclick={() => call('restart_app')}>Şimdi yeniden başlat</button>
      </div>
    {/if}
  </section>

  {#if s.settings.mutedApps.length}
    <section class="card">
      <h2>Susturulan uygulamalar</h2>
      {#each s.settings.mutedApps as pkg (pkg)}
        <div class="row line">
          <span class="grow ellipsis">{pkg}</span>
          <button class="ghost" onclick={() => unmute(pkg)}>Aç</button>
        </div>
      {/each}
    </section>
  {/if}

  <section class="card muted small">
    <div>Bu bilgisayar: {s.deviceName} · UDP {s.port}</div>
    <div class="selectable">Parmak izi: {s.fingerprint}</div>
    <div>Bağlantı uçtan uca şifrelidir (QUIC + TLS 1.3); yalnızca eşleşmiş cihazlar bağlanabilir.</div>
  </section>
</div>

<style>
  .page {
    display: grid;
    gap: 14px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .toggle:last-child {
    border-bottom: none;
  }
  .toggle input {
    width: 38px;
    height: 20px;
    appearance: none;
    background: var(--line);
    border-radius: 99px;
    position: relative;
    cursor: pointer;
    transition: 0.15s;
    flex: none;
  }
  .toggle input::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: 0.15s;
  }
  .toggle input:checked {
    background: var(--accent);
  }
  .toggle input:checked::after {
    left: 20px;
  }
  .dir {
    margin-top: 10px;
  }
  .warn {
    margin-top: 10px;
    color: var(--warn);
  }
  .line {
    padding: 6px 0;
  }
  .small {
    font-size: 12px;
  }
  h2 {
    margin-bottom: 6px;
  }
</style>
