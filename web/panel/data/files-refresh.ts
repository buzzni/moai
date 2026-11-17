import type { Clock, VisibilitySignal } from './refresh.ts'

export interface WatchedFile { path: string; stamp: string | null; readAt?: number }
export interface FileWatch {
  key: string
  dirs: WatchedFile[]
  root?: string
  file?: WatchedFile
  tracker?: boolean
  changeKey?: string
  changeFingerprint?: string
}
export interface StampAnswer { dirs: Record<string, string | null>; file: string | null }
export interface FileRefreshOptions {
  clock: Clock
  visibility: VisibilitySignal
  snapshot(): FileWatch | null
  stamps(watch: FileWatch, dirs: string[]): Promise<StampAnswer>
  changes(watch: FileWatch): Promise<string>
  changedStamps(watch: FileWatch, dirs: string[], file: boolean, answer: StampAnswer): Promise<void>
  changedChanges(watch: FileWatch, fingerprint: string): Promise<void>
  cancel?(): void
  onBackoff?(backingOff: boolean): void
}
/** Quiet steps slow down, and expensive Git reads wait ten times their last duration. */
export function nextDelay(duration: number, quiet: number, failed: boolean): number {
  if (failed) return 10_000
  return Math.max(2_000, duration * 10, Math.min(10_000, 2_000 * (1 + quiet)))
}
export function createFilesRefresher(options: FileRefreshOptions) {
  const { clock, visibility } = options
  let timer: unknown = null, stopped = false, running = false, epoch = 0, rotation = 0, quiet = 0, backingOff = false
  let nextChanges = 0, lastKey = '', lastChangeKey = '', lastFingerprint: string | undefined
  const checkedFresh = new Map<string, number>()
  const checkAgain = new Set<string>()
  function clear(): void { if (timer !== null) clock.clearTimeout(timer); timer = null }
  function schedule(ms: number): void {
    clear(); if (stopped || !visibility.shown() || !options.snapshot()) return
    timer = clock.setTimeout(() => { timer = null; void check() }, ms)
  }
  async function check(): Promise<void> {
    if (running || stopped || !visibility.shown()) return
    const watch = options.snapshot(); if (!watch) return
    const turn = epoch; running = true; let changed = false, failed = false
    if (watch.key !== lastKey) { lastKey = watch.key; rotation = 0; nextChanges = 0; lastChangeKey = ''; checkedFresh.clear(); checkAgain.clear() }
    try {
      const ordered = watch.dirs.length ? [...watch.dirs.slice(rotation), ...watch.dirs.slice(0, rotation)] : []
      const batch = ordered.slice(0, 64); rotation = watch.dirs.length ? (rotation + batch.length) % watch.dirs.length : 0
      if (batch.length || watch.file || watch.tracker) {
        const stamps = await options.stamps(watch, batch.map(d => d.path))
        if (turn !== epoch || stopped || !visibility.shown() || options.snapshot()?.key !== watch.key) return
        const fresh = (item: WatchedFile): boolean => {
          if (item.readAt === undefined || clock.now() - item.readAt >= 2_000 || checkedFresh.get(item.path) === item.readAt) return false
          checkedFresh.set(item.path, item.readAt); return true
        }
        const changedDirs = batch.filter(d => (stamps.dirs[d.path] ?? null) !== d.stamp).map(d => d.path)
        const changedFile = watch.file !== undefined && stamps.file !== watch.file.stamp
        const dirs = batch.filter(d => changedDirs.includes(d.path) || checkAgain.delete(`dir:${d.path}`) || fresh(d)).map(d => d.path)
        const file = watch.file !== undefined && (changedFile || checkAgain.delete(`file:${watch.file.path}`) || fresh(watch.file))
        if (dirs.length || file) {
          changed = true
          try { await options.changedStamps(watch, dirs, file, stamps) }
          catch (error) { for (const path of dirs) checkAgain.add(`dir:${path}`); if (file && watch.file) checkAgain.add(`file:${watch.file.path}`); throw error }
          const latest = options.snapshot()
          if (latest?.key === watch.key) {
            for (const item of latest.dirs) if (dirs.includes(item.path) && item.readAt !== undefined) checkedFresh.set(item.path, item.readAt)
            if (file && latest.file?.readAt !== undefined) checkedFresh.set(latest.file.path, latest.file.readAt)
            for (const path of changedDirs) checkAgain.add(`dir:${path}`)
            if (changedFile && watch.file) checkAgain.add(`file:${watch.file.path}`)
          }
        }
      }
      if (watch.changeKey !== lastChangeKey) { lastChangeKey = watch.changeKey ?? ''; nextChanges = 0; lastFingerprint = watch.changeFingerprint }
      if (watch.changeKey && clock.now() >= nextChanges) {
        const start = clock.now(); const fingerprint = await options.changes(watch); const elapsed = clock.now() - start
        if (turn !== epoch || stopped || !visibility.shown() || options.snapshot()?.changeKey !== watch.changeKey) return
        nextChanges = clock.now() + Math.max(2_000, elapsed * 10)
        const previous = watch.changeFingerprint ?? lastFingerprint
        if (previous !== undefined && fingerprint !== previous) { changed = true; await options.changedChanges(watch, fingerprint) }
        lastFingerprint = fingerprint
      }
    } catch { failed = true }
    finally {
      running = false
      if (!stopped) { if (turn !== epoch) schedule(0); else { if (failed !== backingOff) { backingOff = failed; options.onBackoff?.(failed) } quiet = changed ? 0 : quiet + 1; schedule(nextDelay(0, quiet, failed)) } }
    }
  }
  const unwatch = visibility.watch(() => { epoch++; clear(); if (!visibility.shown()) options.cancel?.(); else schedule(0) })
  return {
    check,
    wake() { epoch++; quiet = 0; clear(); options.cancel?.(); if (!options.snapshot() && backingOff) { backingOff = false; options.onBackoff?.(false) } if (visibility.shown()) schedule(0) },
    touch() { quiet = 0; if (!running) schedule(2_000) },
    stop() { stopped = true; epoch++; clear(); options.cancel?.(); unwatch() },
  }
}
