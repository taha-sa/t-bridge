<script lang="ts">
  import { app, applyTheme, call, flash } from './store.svelte'
  import { t, syncLang } from './i18n/index.svelte'
  import type { Settings, Theme } from './types'
  import Icon from './Icon.svelte'
  import Toggle from './ui/Toggle.svelte'
  import Progress from './ui/Progress.svelte'
  import Logo from './ui/Logo.svelte'
  import Signature from './ui/Signature.svelte'
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import LegalDialog from './LegalDialog.svelte'
  import ReportDialog, { type Preview } from './ReportDialog.svelte'
  import { APPS, type AppId } from './apps'
  import bridgeIcon from '../assets/apps/t-bridge.png'

  const s = $derived(app.s)
  const u = $derived(s.update)
  let restartNeeded = $state(false)
  let checking = $state(false)

  async function save(patch: Partial<Settings>) {
    const next = { ...s.settings, ...patch }
    s.settings = next
    if (patch.notifMirror === false) s.notifications = []
    if (patch.theme) applyTheme(patch.theme)
    const r = await call<boolean>('set_settings', { settings: next })
    if (r) restartNeeded = true
    if (patch.language) await syncLang()
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
    flash(t('set.unmuted'))
  }

  async function revokeScreen(id: string) {
    s.settings.screenAllowed = s.settings.screenAllowed.filter((p) => p !== id)
    await call('revoke_screen_allowed', { id })
    flash(t('set.screenRevoked'))
  }

  async function checkNow() {
    checking = true
    const found = await call<boolean>('check_update')
    checking = false
    if (found === false) flash(t('set.upToDate'))
  }

  // Hakkında: aile uygulamalarının kurulu olup olmadığı (merkez durumundan), yasal metinler, elle hata raporu.
  let installed = $state<Record<string, boolean>>({})
  let legal = $state<'privacy' | 'disclaimer' | 'license' | 'notices' | null>(null)
  let report = $state<Preview | null>(null)
  const family: AppId[] = ['t-downloader', 't-ramcash', 't-desktree', 't-desk-viewer']
  onMount(() => {
    invoke<{ id: string; installed: boolean }[]>('suite_status')
      .then((cs) => (installed = Object.fromEntries(cs.map((c) => [c.id, c.installed]))))
      .catch(() => {})
  })
  async function reportBug() {
    report = (await call<Preview>('report_manual')) ?? null
  }

  async function fixFirewall() {
    const ok = await call<boolean>('fix_firewall')
    s.firewallOk = !!ok
    flash(ok ? t('fw.granted') : t('fw.notGranted'))
  }

  const themes = $derived<{ id: Theme; label: string; icon: string }[]>([
    { id: 'dark', label: t('set.themeDark'), icon: 'moon' },
    { id: 'system', label: t('set.themeSystem'), icon: 'monitor' },
    { id: 'light', label: t('set.themeLight'), icon: 'sun' },
  ])
  // Dil adları kendi dillerinde yazılır (hangi dilde olursa olsun bulunabilsin).
  const languages = $derived<{ id: Settings['language']; label: string }[]>([
    { id: 'system', label: t('set.langSystem') },
    { id: 'tr', label: 'Türkçe' },
    { id: 'en', label: 'English' },
    { id: 'de', label: 'Deutsch' },
  ])

  const updateText = $derived(
    u.phase === 'checking'
      ? t('set.updChecking')
      : u.phase === 'downloading'
        ? t('set.updDownloading', { v: u.version ?? '', p: Math.round(u.progress * 100) })
        : u.phase === 'ready'
          ? t('set.updReady', { v: u.version ?? '' })
          : u.phase === 'installing'
            ? t('set.updInstalling')
            : u.phase === 'latest'
              ? t('set.updLatest')
              : u.phase === 'error'
                ? t('set.updError', { e: u.error ?? t('set.unknownError') })
                : t('set.updIdle'),
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
    <h3>{t('set.general')}</h3>
    <div class="item">
      <div class="grow">
        <div class="il">{t('set.theme')}</div>
        <div class="ih">{t('set.themeHint')}</div>
      </div>
      <div class="seg">
        {#each themes as th (th.id)}
          <button class:sel={s.settings.theme === th.id} onclick={() => save({ theme: th.id })}>
            <Icon name={th.icon} size={14} />{th.label}
          </button>
        {/each}
      </div>
    </div>
    <div class="item">
      <div class="grow">
        <div class="il">{t('set.language')}</div>
        <div class="ih">{t('set.languageHint')}</div>
      </div>
      <div class="seg">
        {#each languages as l (l.id)}
          <button class:sel={s.settings.language === l.id} onclick={() => save({ language: l.id })}>{l.label}</button>
        {/each}
      </div>
    </div>
    {@render item(t('set.autostart'), t('set.autostartHint'), 'autostart')}
  </section>

  <section class="card group">
    <h3>{t('set.clipNotif')}</h3>
    {@render item(t('set.screen'), t('set.screenHint'), 'screenShare')}
    {#if s.settings.screenAllowed.length}
      <div class="sub">
        <div class="ih">{t('set.screenAllowed')}</div>
        <div class="muted-list">
          {#each s.settings.screenAllowed as id (id)}
            <span class="chip">{s.peers.find((p) => p.id === id)?.name ?? id.slice(0, 8)}<button class="ghost icon sm x" title={t('set.revoke')} onclick={() => revokeScreen(id)}><Icon name="x" size={12} /></button></span>
          {/each}
        </div>
      </div>
    {/if}
    {@render item(t('set.autoEco'), t('set.autoEcoHint'), 'autoEcosystem')}
    {@render item(t('set.clipSync'), t('set.clipSyncHint'), 'clipboardSync')}
    {@render item(t('set.notifMirror'), t('set.notifMirrorHint'), 'notifMirror')}
    {#if s.settings.notifMirror}
      <div class="item">
        <div class="grow">
          <div class="il">{t('set.notifSound')}</div>
          <div class="ih">{t('set.notifSoundHint')}</div>
        </div>
        <button class="sm" onclick={() => call('test_notification_sound')}>{t('set.try')}</button>
        <Toggle checked={s.settings.notifSound} label={t('set.notifSound')} onchange={(v) => save({ notifSound: v })} />
      </div>
      {@render item(t('set.winToast'), t('set.winToastHint'), 'notifToast')}
    {/if}
    {@render item(t('set.lowBat'), t('set.lowBatHint'), 'lowBatteryWarn')}
    {#if s.settings.mutedApps.length}
      <div class="sub">
        <div class="ih">{t('set.muted')}</div>
        <div class="muted-list">
          {#each s.settings.mutedApps as pkg (pkg)}
            <span class="chip">{pkg}<button class="ghost icon sm x" title={t('set.unmute')} onclick={() => unmute(pkg)}><Icon name="x" size={12} /></button></span>
          {/each}
        </div>
      </div>
    {/if}
  </section>

  <section class="card group">
    <h3>{t('nav.files')}</h3>
    {@render item(t('set.fileToast'), '', 'transferToast')}
    <div class="item">
      <div class="grow">
        <div class="il">{t('set.inbox')}</div>
        <div class="ih ellipsis selectable" title={s.downloadDir}>{s.downloadDir}</div>
      </div>
      <button class="sm" onclick={() => call('open_downloads')}><Icon name="folder" size={14} /> {t('common.open')}</button>
      <button class="sm" onclick={pickDir}>{t('set.change')}</button>
    </div>
    {#if restartNeeded}
      <div class="notice row">
        <Icon name="info" size={16} />
        <span class="grow small">{t('set.restartNote')}</span>
        <button class="primary sm" onclick={() => call('restart_app')}>{t('set.restart')}</button>
      </div>
    {/if}
  </section>

  <section class="card group gold">
    <h3>{t('set.update')}</h3>
    <div class="item">
      <div class="upd-ico" class:busy={u.phase === 'checking' || u.phase === 'downloading'}><Icon name="refresh" size={18} /></div>
      <div class="grow">
        <div class="il">T-Bridge <span class="num">v{s.version}</span>
          {#if u.phase === 'ready'}<span class="chip gold">{t('set.newReady')}</span>{/if}
        </div>
        <div class="ih" class:bad={u.phase === 'error'}>{updateText}</div>
        {#if u.phase === 'downloading'}<div class="pb"><Progress value={u.progress * 100} active /></div>{/if}
      </div>
      {#if u.phase === 'ready'}
        <button class="primary sm" onclick={() => call('install_update')}>{t('set.installNow')}</button>
      {:else}
        <button class="sm" disabled={checking || u.phase === 'checking' || u.phase === 'downloading' || u.phase === 'installing'} onclick={checkNow}>
          {t('set.checkNow')}
        </button>
      {/if}
    </div>
    {#if u.notes && (u.phase === 'ready' || u.phase === 'downloading')}
      <div class="notes small selectable">{u.notes}</div>
    {/if}
    {@render item(t('set.autoUpdate'), t('set.autoUpdateHint'), 'autoUpdate')}
  </section>

  <section class="card group">
    <h3>{t('set.security')}</h3>
    <div class="item">
      <div class="grow">
        <div class="il">{t('set.firewall')}</div>
        <div class="ih">{s.firewallOk ? t('set.firewallOk') : t('set.firewallNo')}</div>
      </div>
      {#if s.firewallOk}
        <span class="chip ok"><Icon name="check" size={13} /> {t('set.on')}</span>
      {:else}
        <button class="primary sm" onclick={fixFirewall}>{t('common.allow')}</button>
      {/if}
    </div>
    <div class="item">
      <div class="grow">
        <div class="il">{t('set.encrypted')}</div>
        <div class="ih">{t('set.encryptedHint')}</div>
      </div>
      <Icon name="lock" size={18} />
    </div>
  </section>

  <section class="card group">
    <h3>{t('set.diagnostics')}</h3>
    {@render item(t('crash.auto'), t('crash.autoHint'), 'autoCrashReport')}
  </section>

  <section class="card group">
    <h3>{t('about.title')}</h3>
    <div class="item">
      <img src={bridgeIcon} alt="" width="36" height="36" class="fic" />
      <div class="grow">
        <div class="il">T-Bridge</div>
        <div class="ih">{t('about.version', { v: s.version })} · {t('about.family')}</div>
      </div>
    </div>
    {#each family as id (id)}
      <div class="item fam">
        <img src={APPS[id].icon} alt="" width="28" height="28" class="fic" />
        <div class="grow il">{APPS[id].name}</div>
        {#if id === 't-desk-viewer'}
          <span class="chip">{t('about.phoneApp')}</span>
        {/if}
        {#if id !== 't-desk-viewer' && installed[id]}
          <span class="chip ok"><Icon name="check" size={13} /> {t('about.installed')}</span>
        {:else}
          <button class="ghost sm" onclick={() => invoke('open_url', { url: APPS[id].url })}>{t(`about.get.${id}`)}</button>
        {/if}
      </div>
    {/each}
    <div class="item">
      <div class="grow"></div>
      <button class="ghost sm" onclick={reportBug}>{t('about.reportBug')}</button>
    </div>
    <div class="sub">
      <div class="il">{t('about.legal')}</div>
      <div class="muted-list">
        <button class="ghost sm" onclick={() => (legal = 'privacy')}>{t('about.privacy')}</button>
        <button class="ghost sm" onclick={() => (legal = 'license')}>{t('about.license')}</button>
        <button class="ghost sm" onclick={() => (legal = 'disclaimer')}>{t('about.disclaimer')}</button>
        <button class="ghost sm" onclick={() => (legal = 'notices')}>{t('about.notices')}</button>
      </div>
    </div>
  </section>

  {#if legal}<LegalDialog kind={legal} onclose={() => (legal = null)} />{/if}
  {#if report}<ReportDialog preview={report} manual={true} onclose={() => (report = null)} />{/if}

  <div class="about">
    <Logo size={42} />
    <div class="an">T-Bridge</div>
    <div class="faint small">{t('set.tagline')} · v{s.version}</div>
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
  .fic {
    border-radius: 8px;
    flex: none;
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
