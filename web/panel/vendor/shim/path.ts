export const sep = '/'
export function isAbsolute(p: string): boolean { return p.startsWith('/') }
export function normalize(p: string): string {
  const absolute = isAbsolute(p), parts: string[] = []
  for (const part of p.split('/')) {
    if (!part || part === '.') continue
    if (part === '..' && parts.length && parts[parts.length - 1] !== '..') parts.pop()
    else if (part === '..' && !absolute) parts.push(part)
    else if (part !== '..') parts.push(part)
  }
  return (absolute ? '/' : '') + parts.join('/') || (absolute ? '/' : '.')
}
export function resolve(...paths: string[]): string {
  let path = ''
  for (let i = paths.length - 1; i >= 0; i--) { const part = paths[i] || ''; path = part + '/' + path; if (isAbsolute(part)) break }
  return normalize(isAbsolute(path) ? path : '/' + path)
}
export function join(...paths: string[]): string { return normalize(paths.filter(Boolean).join('/')) }
export function dirname(path: string): string { const p = normalize(path); const i = p.lastIndexOf('/'); return i < 0 ? '.' : i === 0 ? '/' : p.slice(0, i) }
export function basename(path: string): string { return normalize(path).split('/').pop() || '' }
export function extname(path: string): string { const b = basename(path), i = b.lastIndexOf('.'); return i <= 0 ? '' : b.slice(i) }
export function relative(from: string, to: string): string { const a = resolve(from).split('/').filter(Boolean), b = resolve(to).split('/').filter(Boolean); while (a.length && b.length && a[0] === b[0]) { a.shift(); b.shift() } return [...a.map(() => '..'), ...b].join('/') }
export default { sep, isAbsolute, normalize, resolve, join, dirname, basename, extname, relative }
