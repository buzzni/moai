/** A conservative mirror of image::resolve. The Rust endpoint is the actual file boundary. */
export function imageUrl(doc: Document, src: string, wiki?: { page: string; dir: string }): string | null {
  if (!src || /[\\:%\u0000-\u001f\u007f]/u.test(src) || src.startsWith('/')) return null
  src = src.replace(/^\.\//, '')
  let path: string
  if (!wiki) {
    if (!/^\.moai\/images\/[0-9a-f]{16}\.(png|jpg|gif|webp)$/.test(src)) return null
    path = src
  } else {
    const normalize = (s: string): string[] | null => {
      const parts: string[] = []
      for (const p of s.split('/')) {
        if (!p || p === '.') continue
        if (p === '..') { if (!parts.length) return null; parts.pop() }
        else parts.push(p)
      }
      return parts
    }
    if (/[\\:%]/.test(wiki.dir) || wiki.dir.startsWith('/')) return null
    const root = normalize(wiki.dir)
    const page = normalize(wiki.page)
    if (!root || !page || !root.every((p, i) => page[i] === p)) return null
    const joined = normalize([...page.slice(0, -1), src].join('/'))
    if (!joined || joined.length <= root.length || !root.every((p, i) => joined[i] === p) || joined.some(p => p === '.git' || p === '.moai')) return null
    path = joined.join('/')
    if (!/\.(png|jpg|jpeg|gif|webp)$/i.test(path)) return null
  }
  const query = new URLSearchParams({ path })
  const project = new URLSearchParams(doc.defaultView?.location.search ?? '').get('project')
  if (project) query.set('project', project)
  return `/api/image?${query}`
}
