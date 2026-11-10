// Yasal metinler için küçük, güvenli Markdown çevirici: başlık (###), madde işareti, numaralı liste, **kalın**, `kod`, [bağlantı](https://…).
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

function inline(s: string): string {
  return esc(s)
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\[([^\]]+)\]\((https:\/\/[^)\s]+)\)/g, '<a href="$2" data-url="$2">$1</a>')
}

export function renderMd(src: string): string {
  const out: string[] = []
  let list: 'ul' | 'ol' | null = null
  const close = () => {
    if (list) out.push(`</${list}>`)
    list = null
  }
  for (const raw of src.replace(/\r\n/g, '\n').split('\n')) {
    const line = raw.trimEnd()
    const h = /^(#{1,4})\s+(.*)$/.exec(line)
    const ul = /^\s*[*-]\s+(.*)$/.exec(line)
    const ol = /^\s*\d+\.\s+(.*)$/.exec(line)
    if (h) {
      close()
      out.push(`<h4>${inline(h[2])}</h4>`)
    } else if (ul || ol) {
      const kind = ul ? 'ul' : 'ol'
      if (list !== kind) {
        close()
        out.push(`<${kind}>`)
        list = kind
      }
      out.push(`<li>${inline((ul ?? ol)![1])}</li>`)
    } else if (/^\s+\S/.test(raw) && list) {
      // Madde devam satırı.
      out[out.length - 1] = out[out.length - 1].replace(/<\/li>$/, ` ${inline(line.trim())}</li>`)
    } else if (line.trim() === '' || /^-{3,}$/.test(line.trim())) {
      close()
    } else {
      close()
      out.push(`<p>${inline(line.trim())}</p>`)
    }
  }
  close()
  return out.join('')
}
