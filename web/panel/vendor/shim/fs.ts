export function realpathSync(path: string): string { if (path === '/') return path; throw new Error('not found') }
export function statSync(path: string): { isDirectory(): boolean, isFile(): boolean } | undefined {
  if (path === '/') return { isDirectory: () => true, isFile: () => false }
  return undefined
}
export function readFileSync(): never { throw new Error('not found') }
