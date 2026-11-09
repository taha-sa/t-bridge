// Sürüm derlemesi: sanal kamera DLL'ini üretip Tauri kaynağı olarak src-tauri/camera/ altına koyar.
import { execSync } from 'node:child_process'
import { copyFileSync, mkdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
execSync('cargo build -p tbridge-vcam --release', { cwd: root, stdio: 'inherit' })
const src = join(root, 'target', 'release', 'tbridge_vcam.dll')
const dst = join(root, 'desktop', 'src-tauri', 'camera', 'tbridge_vcam.dll')
mkdirSync(dirname(dst), { recursive: true })
copyFileSync(src, dst)
console.log(`tbridge_vcam.dll -> ${dst} (${statSync(dst).size} bayt)`)
