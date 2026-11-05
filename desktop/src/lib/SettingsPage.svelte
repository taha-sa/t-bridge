<script lang="ts">
  import { app, applyTheme, call, flash } from './store.svelte'
  import type { Settings, Theme } from './types'
  import Icon from './Icon.svelte'
  import Toggle from './ui/Toggle.svelte'
  import Progress from './ui/Progress.svelte'
  import Logo from './ui/Logo.svelte'
  import Signature from './ui/Signature.svelte'

  const s = $derived(app.s)
  const u = $derived(s.update)
  let restartNeeded = $state(false)
  let checking = $state(false)

  async function save(patch: Partial<Settings>) {
    const next = { ...s.settings, ...patch }
    s.settings = next
    if (patch.theme) applyTheme(patch.theme)
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

  async function checkNow() {
    checking = true
    const found = await call<boolean>('check_update')
    checking = false
    if (found === false) flash('T-Bridge güncel')
  }

  async function fixFirewall() {
    const ok = await call<boolean>('fix_firewall')
    s.firewallOk = !!ok
    flash(ok ? 'Güvenlik duvarı izni verildi' : 'İzin verilmedi')
  }

  const themes: { id: Theme; label: string; icon: string }[] = [
    { id: 'dark', label: 'Koyu', icon: 'moon' },
    { id: 'system', label: 'Sistem', icon: 'monitor' },
    { id: 'light', label: 'Açık', icon: 'sun' },
  ]

  const updateText = $derived(
    u.phase === 'checking'
      ? 'Denetleniyor…'
      : u.phase === 'downloading'
        ? `v${u.version} indiriliyor… %${Math.round(u.progress * 100)}`
        : u.phase === 'ready'
          ? `v${u.version} hazır — aktarım bitince kendiliğinden kurulur`
          : u.phase === 'installing'
            ? 'Kuruluyor; T-Bridge birazdan yeniden açılacak…'
            : u.phase === 'latest'
              ? 'En güncel sürümü kullanıyorsun'
              : u.phase === 'error'
                ? `Denetlenemedi: ${u.error ?? 'bilinmeyen hata'}`
                : 'Güncellemeler arka planda denetlenir',
  )
</script>

{#snippet item(label: string, hint: string, key: keyof Settings)}
  <div class="item">
    <div class="grow">
      <div class="il">{label}</div>
      {#if hint}<div class="ih">{hint}</div>{/if}
    </div>
    <Toggle checked={s.settings[key] as boolean} label={label} onchange={(v) => save({ [key]: v })} />
  </div>
{/snippet}

<div class="page narrow">
  <section class="card group">
    <h3>Genel</h3>
    <div class="item">
      <div class="grow">
        <div class="il">Tema</div>
        <div class="ih">Koyu tema T-Bridge için tasarlandı.</div>
      </div>
      <div class="seg">
        {#each themes as t (t.id)}
          <button class:sel={s.settings.theme === t.id} onclick={() => save({ theme: t.id })}>
            <Icon name={t.icon} size={14} />{t.label}
          </button>
        {/each}
      </div>
    </div>
    {@render item('Windows açılışında başlat', 'Tepside sessizce çalışır; pencere açılmaz.', 'autostart')}
  </section>

  <section class="card group">
    <h3>Pano ve bildirimler</h3>
    {@render item('Pano eşitleme', 'Kopyaladığın metin telefonda da kopyalanmış olur.', 'clipboardSync')}
    {@render item('Bildirimleri Windows’ta göster', 'Pencere arkadayken telefon bildirimleri sağ altta çıkar.', 'notifToast')}
    {@render item('Şarj azalınca uyar', 'Telefon %15’in altına inince haber verir.', 'lowBatteryWarn')}
    {#if s.settings.mutedApps.length}
      <div class="sub">
        <div class="ih">Susturulan uygulamalar</div>
        <div class="muted-list">
          {#each s.settings.mutedApps as pkg (pkg)}
            <span class="chip">{pkg}<button class="ghost icon sm x" title="Tekrar aç" onclick={() => unmute(pkg)}><Icon name="x" size={12} /></button></span>
          {/each}
        </div>
      </div>
    {/if}
  </section>

  <section class="card group">
    <h3>Dosyalar</h3>
    {@render item('Dosya alınınca haber ver', '', 'transferToast')}
    <div class="item">
      <div class="grow">
        <div class="il">Gelen dosyalar klasörü</div>
        <div class="ih ellipsis selectable" title={s.downloadDir}>{s.downloadDir}</div>
      </div>
      <button class="sm" onclick={() => call('open_downloads')}><Icon name="folder" size={14} /> Aç</button>
      <button class="sm" onclick={pickDir}>Değiştir…</button>
    </div>
    {#if restartNeeded}
      <div class="notice row">
        <Icon name="info" size={16} />
        <span class="grow small">Yeni klasör, uygulama yeniden başlayınca kullanılır.</span>
        <button class="primary sm" onclick={() => call('restart_app')}>Yeniden başlat</button>
      </div>
    {/if}
  </section>

  <section class="card group gold">
    <h3>Güncelleme</h3>
    <div class="item">
      <div class="upd-ico" class:busy={u.phase === 'checking' || u.phase === 'downloading'}><Icon name="refresh" size={18} /></div>
      <div class="grow">
        <div class="il">T-Bridge <span class="num">v{s.version}</span>
          {#if u.phase === 'ready'}<span class="chip gold">yeni sürüm hazır</span>{/if}
        </div>
        <div class="ih" class:bad={u.phase === 'error'}>{updateText}</div>
        {#if u.phase === 'downloading'}<div class="pb"><Progress value={u.progress * 100} active /></div>{/if}
      </div>
      {#if u.phase === 'ready'}
        <button class="primary sm" onclick={() => call('install_update')}>Şimdi kur</button>
      {:else}
        <button class="sm" disabled={checking || u.phase === 'checking' || u.phase === 'downloading' || u.phase === 'installing'} onclick={checkNow}>
          Şimdi denetle
        </button>
      {/if}
    </div>
    {#if u.notes && (u.phase === 'ready' || u.phase === 'downloading')}
      <div class="notes small selectable">{u.notes}</div>
    {/if}
    {@render item('Otomatik güncelle', 'Yeni sürüm arka planda iner, aktarım yokken sessizce kurulur. Telefon da güncellemeyi kendisi ister.', 'autoUpdate')}
  </section>

  <section class="card group">
    <h3>Güvenlik</h3>
    <div class="item">
      <div class="grow">
        <div class="il">Güvenlik duvarı</div>
        <div class="ih">{s.firewallOk ? 'İzin verildi — telefon bu bilgisayara ulaşabilir.' : 'İzin yok — telefon bağlanamayabilir.'}</div>
      </div>
      {#if s.firewallOk}
        <span class="chip ok"><Icon name="check" size={13} /> Açık</span>
      {:else}
        <button class="primary sm" onclick={fixFirewall}>İzin ver</button>
      {/if}
    </div>
    <div class="item">
      <div class="grow">
        <div class="il">Şifreleme</div>
        <div class="ih">QUIC + TLS 1.3, cihaz sertifikası sabitleme. Yalnızca eşleşmiş cihazlar bağlanır; veri ağın dışına çıkmaz.</div>
      </div>
      <Icon name="lock" size={18} />
    </div>
    <div class="fp faint small num selectable">{s.deviceName} · UDP {s.port} · {s.fingerprint.match(/.{1,8}/g)?.slice(0, 4).join(' ')}</div>
  </section>

  <div class="about">
    <Logo size={42} />
    <div class="an">T-Bridge</div>
    <div class="faint small">Telefon ile bilgisayar arasında köprü · v{s.version}</div>
  </div>
  <Signature />
</div>

<style>
  .narrow {
    max-width: 760px;
  }
  .group {
    padding: 18px 20px 8px;
  }
  .group h3 {
    margin-bottom: 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 0;
  }
  .item + .item {
    border-top: 1px solid var(--line);
  }
  .il {
    font-weight: 560;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ih {
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 2px;
  }
  .ih.bad {
    color: var(--bad);
  }
  .seg {
    display: flex;
    background: var(--s3);
    border: 1px solid var(--line);
    border-radius: 11px;
    padding: 3px;
    gap: 2px;
  }
  .seg button {
    height: 28px;
    padding: 0 11px;
    font-size: 12.5px;
    border: none;
    background: transparent;
    border-radius: 8px;
    color: var(--muted);
  }
  .seg button:hover:not(:disabled) {
    background: var(--s4);
    border: none;
    color: var(--text);
  }
  .seg button.sel {
    background: var(--gold-soft);
    color: var(--gold-hi);
    box-shadow: inset 0 0 0 1px var(--gold-line);
  }
  .sub {
    padding: 12px 0 14px;
    border-top: 1px solid var(--line);
  }
  .muted-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .muted-list .chip {
    padding-right: 2px;
  }
  .x {
    width: 20px;
    height: 20px;
    border-radius: 50%;
  }
  .notice {
    gap: 10px;
    padding: 10px 12px;
    margin-bottom: 12px;
    border-radius: 12px;
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
    color: var(--text);
  }
  .upd-ico {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    flex: none;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  .upd-ico.busy :global(svg) {
    animation: spin 1.1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .pb {
    margin-top: 8px;
    max-width: 320px;
  }
  .notes {
    white-space: pre-wrap;
    color: var(--text-2);
    padding: 10px 12px;
    margin: -4px 0 12px 52px;
    border-radius: 10px;
    background: var(--s3);
  }
  .fp {
    padding: 4px 0 12px;
    word-break: break-all;
  }
  .about {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding-top: 22px;
  }
  .an {
    font-family: 'Segoe UI Variable Display', 'Segoe UI', sans-serif;
    font-size: 18px;
    font-weight: 680;
    margin-top: 4px;
  }
</style>
