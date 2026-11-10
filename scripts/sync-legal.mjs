// Yasal metin kaynağından (taslak-yasal-metinler.md) LICENSE, PRIVACY.md ve uygulama içi metinleri üretir.
// Kullanım: node scripts/sync-legal.mjs <taslak-yasal-metinler.md>
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const src = readFileSync(process.argv[2], 'utf8').replace(/\r\n/g, '\n')
const parts = {}
let key = null
for (const line of src.split('\n')) {
  const m = /^# (.+)$/.exec(line)
  if (m) { key = m[1].trim(); parts[key] = []; continue }
  if (key) parts[key].push(line)
}
const body = (k) => {
  const v = Object.entries(parts).find(([n]) => n.startsWith(k))
  if (!v) throw new Error('bölüm yok: ' + k)
  return v[1].join('\n').replace(/\n---\s*(\n---\s*)*$/g, '').trim() + '\n'
}
const license = body('LICENSE').replace('Copyright (c) 2026 taha (https://github.com/taha-sa)', 'Copyright (c) 2026 taha (taha-sa)')
writeFileSync(join(root, 'LICENSE'), license)
mkdirSync(join(root, 'desktop/src/lib/legal'), { recursive: true })
writeFileSync(join(root, 'desktop/src/lib/legal/LICENSE.txt'), license)
mkdirSync(join(root, 'android/app/src/main/assets/legal'), { recursive: true })
writeFileSync(join(root, 'android/app/src/main/assets/legal/LICENSE.txt'), license)

const L = { tr: 'TR', en: 'EN', de: 'DE' }
const names = {
  privacy: { tr: 'GİZLİLİK POLİTİKASI', en: 'PRIVACY POLICY', de: 'DATENSCHUTZERKLÄRUNG' },
  disclaimer: { tr: 'KULLANIM NOTU', en: 'DISCLAIMER', de: 'HAFTUNGSAUSSCHLUSS' },
}
let privacyAll = '# T-Bridge · Gizlilik / Privacy / Datenschutz\n\n'
for (const kind of ['privacy', 'disclaimer']) {
  for (const l of ['tr', 'en', 'de']) {
    const text = body(names[kind][l])
    for (const dir of ['desktop/src/lib/legal', 'android/app/src/main/assets/legal']) {
      mkdirSync(join(root, dir), { recursive: true })
      writeFileSync(join(root, dir, `${kind}.${l}.md`), text)
    }
    if (kind === 'privacy') privacyAll += `## ${names.privacy[l]} (${L[l]})\n\n${text.replace(/^### /gm, '#### ')}\n---\n\n`
  }
}
writeFileSync(join(root, 'PRIVACY.md'), privacyAll.trimEnd() + '\n')
console.log('tamam')
