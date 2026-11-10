// Açık kaynak bildirimleri: Rust (cargo metadata / Cargo.lock), npm (package-lock.json), Gradle (en iyi çaba).
// Çıktı: desktop/src/lib/legal/notices.json (Rust + npm) ve android/app/src/main/assets/legal/notices.json (Rust + Gradle).
// Biçim: [[ad, sürüm, lisans, ekosistem]]. Çalıştırma: node scripts/gen-notices.mjs
// Gradle: build.gradle.kts içindeki "grup:ad:sürüm" bağımlılıkları okunur; lisans yalnız bilinen gruplar için doldurulur
// (androidx/com.google → Apache-2.0, zxing → Apache-2.0, jna → Apache-2.0 / LGPL-2.1, kotlinx → Apache-2.0), aksi halde "?".
import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const meta = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], { cwd: root, maxBuffer: 1 << 28, encoding: 'utf8' }))
const rust = meta.packages
  .filter((p) => p.source) // çalışma alanı üyeleri hariç
  .map((p) => [p.name, p.version, p.license || (p.license_file ? 'see license file' : '?'), 'rust'])

const lock = JSON.parse(readFileSync(join(root, 'desktop/package-lock.json'), 'utf8'))
const npm = Object.entries(lock.packages)
  .filter(([k, p]) => k && (!p.dev || k === 'node_modules/svelte'))
  .map(([k, p]) => [k.replace(/^.*node_modules\//, ''), p.version, p.license || '?', 'npm'])

const gradle = []
const kts = readFileSync(join(root, 'android/app/build.gradle.kts'), 'utf8')
const vars = Object.fromEntries([...kts.matchAll(/val\s+(\w+)\s*=\s*"([^"]+)"/g)].map((m) => [m[1], m[2]]))
for (const m of kts.matchAll(/implementation\("([^"]+)"\)/g)) {
  const [g, a, v = ''] = m[1].replace(/@\w+$/, '').split(':')
  const ver = v.replace(/\$(\w+)/g, (_, k) => vars[k] ?? '')
  const lic = g === 'net.java.dev.jna' ? 'Apache-2.0 OR LGPL-2.1-or-later' : /^(androidx|com\.google|org\.jetbrains)/.test(g) ? 'Apache-2.0' : '?'
  gradle.push([`${g}:${a}`, ver, lic, 'gradle'])
}
const sort = (a) => a.sort((x, y) => x[0].localeCompare(y[0]) || x[1].localeCompare(y[1]))
const out = (dir, arr) => {
  mkdirSync(join(root, dir), { recursive: true })
  writeFileSync(join(root, dir, 'notices.json'), JSON.stringify(sort(arr)) + '\n')
  console.log(dir, arr.length)
}
out('desktop/src/lib/legal', [...rust, ...npm])
out('android/app/src/main/assets/legal', [...rust, ...gradle])
