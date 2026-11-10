#!/usr/bin/env node
// T-Bridge yayın betiği.
//   node scripts/release.mjs 0.2.1 "Sürüm notu"            → derle, doğrula, GitHub'a yayınla
//   node scripts/release.mjs 0.2.1 "Sürüm notu" --no-publish → yalnızca derle ve dist-release/ altına koy
//   node scripts/release.mjs 0.3.0-beta.1 "not" --ci        → CI modu (bkz. docs/CI.md): doğrulamayı atlar (CI yaptı),
//     anahtarları ortamdan okur (TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]), git commit/tag yapmaz, `gh` PATH'ten çalışır.
//     Beta (-beta.N): ön sürüm olarak yayınlanır; latest.json/android.json YÜKLENMEZ (güncelleyiciler yalnızca kararlı kanalı izler).
// Gerekenler: ~/.tauri/tbridge.key + tbridge.pass (güncelleme imzası), android/keystore.properties (APK imzası), gh girişi.
import { execSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const REPO = 'taha-sa/t-bridge-releases'
const ci = process.argv.includes('--ci')
const GH = process.env.GH_BIN || (ci ? 'gh' : 'C:\\Program Files\\GitHub CLI\\gh.exe')
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

const [tagVersion, notes = ''] = process.argv.slice(2).filter((a) => !a.startsWith('--'))
const publish = !process.argv.includes('--no-publish')
const m = /^(\d+\.\d+\.\d+)(-beta\.?\d+)?$/.exec(tagVersion ?? '')
if (!m) {
  console.error('Kullanım: node scripts/release.mjs <x.y.z | x.y.z-beta.N> "not" [--no-publish] [--ci]')
  process.exit(1)
}
const version = m[1] // dosyalara ve manifestlere yazılan x.y.z
const beta = Boolean(m[2])
const tag = `v${tagVersion}` // GitHub etiketi (beta son eki dahil)

const run = (cmd, cwd = root, env = {}) => {
  console.log(`\n$ ${cmd}`)
  execSync(cmd, { cwd, stdio: 'inherit', env: { ...process.env, ...env } })
}
const edit = (path, fn) => {
  const p = join(root, path)
  writeFileSync(p, fn(readFileSync(p, 'utf8')))
}
const sha256 = (p) => createHash('sha256').update(readFileSync(p)).digest('hex')

// Eski sürüm yayınlanmasın: telefon ve PC yalnızca daha yeni sürüme geçer.
const prev = readFileSync(join(root, 'version.txt'), 'utf8').trim()
const code = (v) => v.split('.').reduce((a, n, i) => a + Number(n) * [10000, 100, 1][i], 0)
if (code(version) < code(prev)) {
  console.error(`Sürüm geriye gidemez: ${prev} → ${version}`)
  process.exit(1)
}

// 1) Sürüm numarasını tek kaynaktan her yere yaz.
writeFileSync(join(root, 'version.txt'), `${version}\n`)
edit('desktop/src-tauri/tauri.conf.json', (s) => s.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`))
for (const f of ['core/Cargo.toml', 'desktop/src-tauri/Cargo.toml']) {
  edit(f, (s) => s.replace(/^version = "[^"]+"/m, `version = "${version}"`))
}

// 2) Doğrulama: kırık sürüm yayınlanmaz.
if (!ci) {
  run('cargo test -p kopru-core --release')
  run('cargo clippy --workspace --release -- -D warnings')
  run('npx svelte-check --tsconfig ./tsconfig.app.json --threshold error', join(root, 'desktop'))
}

// 3) Masaüstü: imzalı NSIS kurulumu + .sig
const keyPath = join(homedir(), '.tauri', 'tbridge.key')
const passPath = join(homedir(), '.tauri', 'tbridge.pass')
run('npx tauri build', join(root, 'desktop'), {
  TAURI_SIGNING_PRIVATE_KEY: ci ? process.env.TAURI_SIGNING_PRIVATE_KEY : readFileSync(keyPath, 'utf8'),
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ci ? (process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? '') : readFileSync(passPath, 'utf8'),
})
const nsisDir = join(root, 'target', 'release', 'bundle', 'nsis')
const setupName = `T-Bridge_${version}_x64-setup.exe`
const setup = join(nsisDir, setupName)
const sig = `${setup}.sig`
if (!existsSync(setup) || !existsSync(sig)) throw new Error(`Kurulum veya imza bulunamadı: ${setup}`)

// 4) Android: imzalı APK
if (!existsSync(join(root, 'android', 'keystore.properties'))) throw new Error('android/keystore.properties yok: APK güncellenebilir imzalanamaz')
run(`"${join(root, 'android', 'gradlew.bat')}" --no-daemon -q lintRelease assembleRelease`, join(root, 'android'))
const apkSrc = join(root, 'android', 'app', 'build', 'outputs', 'apk', 'release', 'app-release.apk')

// 5) Yayın klasörü ve bildirim dosyaları
const out = join(root, 'dist-release', tagVersion)
rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })
const apkName = `T-Bridge-${version}.apk`
copyFileSync(setup, join(out, setupName))
copyFileSync(sig, join(out, `${setupName}.sig`))
copyFileSync(apkSrc, join(out, apkName))

const base = `https://github.com/${REPO}/releases/download/${tag}`
if (!beta) writeFileSync(
  join(out, 'latest.json'),
  JSON.stringify(
    {
      version,
      notes,
      pub_date: new Date().toISOString(),
      platforms: { 'windows-x86_64': { signature: readFileSync(sig, 'utf8').trim(), url: `${base}/${setupName}` } },
    },
    null,
    2,
  ),
)
if (!beta) writeFileSync(
  join(out, 'android.json'),
  JSON.stringify(
    { versionCode: code(version), versionName: version, url: `${base}/${apkName}`, sha256: sha256(join(out, apkName)), notes },
    null,
    2,
  ),
)
console.log(`\nYayın dosyaları: ${out}`)

if (!publish) process.exit(0)

// 6) Git ve GitHub Releases
if (!ci) run('git add -A')
if (!ci) try {
  execSync(`git commit -q -m "Sürüm ${version}" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"`, { cwd: root, stdio: 'inherit' })
} catch {
  console.log('(işlenecek değişiklik yok)')
}
if (!ci) run(`git tag -f ${tag}`)
const notesFile = join(out, 'NOTES.txt')
writeFileSync(notesFile, notes || `T-Bridge ${tagVersion}`)
const assets = [setupName, `${setupName}.sig`, apkName, ...(beta ? [] : ['latest.json', 'android.json'])].map((f) => `"${join(out, f)}"`).join(' ')
run(`"${GH}" release create ${tag} ${assets} --repo ${REPO} --title "T-Bridge ${tagVersion}" --notes-file "${notesFile}" ${beta ? '--prerelease' : '--latest'}`)
console.log(`\nYayınlandı: https://github.com/${REPO}/releases/tag/${tag}`)
