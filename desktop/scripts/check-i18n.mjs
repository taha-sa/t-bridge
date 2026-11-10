// Arayüz dil dosyalarını denetler: tr/en/de anahtar kümeleri birebir aynı, değerler boş değil,
// yer tutucular ({ad}) dillerde aynı, çoğul çiftleri (_one/_other) tam. Hata varsa çıkış kodu 1.
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const dir = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'lib', 'i18n')
const langs = ['tr', 'en', 'de']
const dicts = Object.fromEntries(langs.map((l) => [l, JSON.parse(readFileSync(join(dir, `${l}.json`), 'utf8'))]))
const errors = []
const ph = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')

const all = new Set(langs.flatMap((l) => Object.keys(dicts[l])))
for (const k of all) {
  for (const l of langs) {
    const v = dicts[l][k]
    if (v === undefined) errors.push(`[${l}] eksik anahtar: ${k}`)
    else if (typeof v !== 'string' || v.trim() === '') errors.push(`[${l}] boş değer: ${k}`)
    else if (/!/.test(v) && !/<[^>]*!/.test(v)) errors.push(`[${l}] ünlem işareti: ${k}`)
  }
  const base = dicts.tr[k]
  if (typeof base === 'string') {
    for (const l of langs) {
      const v = dicts[l][k]
      if (typeof v === 'string' && ph(v) !== ph(base)) errors.push(`[${l}] yer tutucu uyuşmuyor: ${k}`)
    }
  }
  const m = k.match(/^(.*)_(one|other)$/)
  if (m && !all.has(`${m[1]}_${m[2] === 'one' ? 'other' : 'one'}`)) errors.push(`çoğul çifti eksik: ${k}`)
}

if (errors.length) {
  console.error(errors.join('\n'))
  console.error(`i18n denetimi BAŞARISIZ (${errors.length} hata)`)
  process.exit(1)
}
console.log(`i18n denetimi tamam: ${all.size} anahtar x ${langs.length} dil`)
