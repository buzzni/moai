/**
 * Compatibility: what the panel can tell about the `moai` CLI and the Happy daemon's catalog from what they
 * answered, so a banner can say "upgrade moai" or "update the daemon catalog" instead of a broken screen.
 * No DOM here, and nothing in this module asks anything: loaders hand it the outcomes and shapes they already
 * read ({@link CompatProbe}), and {@link CompatProbe.report} judges them. The probe's listeners
 * ({@link CompatProbe.subscribe}) hear of each thing handed over, a late one too, so the caller can put the new
 * report where it is drawn. The banner is drawn elsewhere.
 *
 * Extension and moai versions are independent (README, Compatibility); the extension relies on the upstream
 * improvements moai 0.17.0 brought (se-0g4a) and assumes a catalog of 12 profiles (se-0g4a.anz). Features that
 * still work keep working: the fallbacks are in source.ts, this only names which ones are in use.
 *
 * What is evidence of what:
 * - `refused` `unsupported-daemon` on a profile: the daemon's catalog does not have it, or the daemon is too
 *   old for the machine run at all. Only a profile that ran tells the two apart: until one has, and whenever a
 *   baseline profile is refused as unsupported, the catalog is `unknown` as a whole, not 12 missing ones.
 *   `tool-missing`: `moai` is not installed there.
 * - A start that failed comes back `not-started` (client.ts), with the code it named when it named one: an older
 *   host may name `unsupported-daemon` in a thrown start's message, which is read as the refusal is. A profile
 *   read as missing stays so until one pass of the same profile clears it. `binding-mismatch` (a refusal the
 *   panel does not list; the extension answers every SDK refusal code, src/index.ts) says the binding or the
 *   approval changed, nothing of the catalog: it counts as a start that failed, as one naming no code does.
 * - The profiles 2.0 added (se-0g4a.anz: the whole list, the backlog, the archive, the read marks) reach a daemon's
 *   catalog together. Once the whole list is read as missing, the rest of them are taken as missing too and not run
 *   ({@link CompatProbe.lacks}): Desktop puts its confirmation up before it asks the daemon, so each would ask the
 *   person to approve a command the daemon then refuses (se-24ey.dcj). The whole list itself is still run, so a
 *   catalog updated meanwhile is found on the next read.
 * - The shape of `status --json` is the strong sign of an old moai: new moai prints `statuses[]` and the
 *   `milestones[]`/`epics[]` tallies, old moai (0.16) prints `counts` and no `statuses`.
 * - `degraded` names only what the panel uses ({@link ORDER}): moai 0.16 prints the tallies, and `statuses[]` it lacks
 *   serves nothing drawn yet, so a 0.16 reads `old` with nothing degraded. NEW marks are not judged: moai 0.16 prints
 *   `unread` too, and the key is absent whenever nothing is unread, so its absence tells "nothing unread" from
 *   "moai gives no unread" never (se-xee6).
 */
import { refusalCode, type Outcome } from './client.js'
import { PROFILE } from './source.js'

export type Feature = 'all-rows' | 'backlog' | 'archive' | 'wiki' | 'read-marks' | 'status-order' | 'tallies'

export interface CompatReport {
  /** `old`: a shape only an older moai prints. `unknown`: nothing seen decides it (or `moai` is missing). */
  moai: 'ok' | 'old' | 'unknown'
  /** `moai` is not installed where the daemon runs: a `tool-missing` refusal, and no run started there since. */
  toolMissing: boolean
  catalog: {
    /** `missing`: `missing` names profile ids; `unknown`: nothing has run yet, or the daemon refused a baseline profile. */
    state: 'ok' | 'missing' | 'unknown'
    missing: string[]
  }
  /** Features the panel uses that run on a fallback or are off, in a fixed order. */
  degraded: Feature[]
}

/** The profiles a daemon of the shipped catalog already has (source.ts header); the rest are the assumed ones. */
const BASELINE: ReadonlySet<string> = new Set([PROFILE.status, PROFILE.prime, PROFILE.show, PROFILE.wikiLs, PROFILE.wikiShow])

/** The profiles 2.0 added, which a catalog gains together: one of them missing tells the others' state. */
const ADDED: ReadonlySet<string> = new Set([
  PROFILE.showAll, PROFILE.backlog, PROFILE.showArchived, PROFILE.showArchivedAfter, PROFILE.read, PROFILE.readAll, PROFILE.readGroup,
])

const FEATURE_OF: Readonly<Record<string, Feature>> = {
  [PROFILE.showAll]: 'all-rows',
  [PROFILE.backlog]: 'backlog',
  [PROFILE.showArchived]: 'archive',
  [PROFILE.showArchivedAfter]: 'archive',
  [PROFILE.wikiLs]: 'wiki',
  [PROFILE.wikiShow]: 'wiki',
  [PROFILE.read]: 'read-marks',
  [PROFILE.readAll]: 'read-marks',
  [PROFILE.readGroup]: 'read-marks',
}

/**
 * The features the panel uses, in the banner's order; one judged off that is not here is left out of `degraded`.
 * `unread` (NEW marks: read.ts isUnread) is drawn, but is not here: a missing key cannot be told from nothing unread.
 * `status-order` (the columns from `status`: source.ts loadStatuses) is told apart below but has no caller yet: put
 * it back here when it is drawn.
 */
const ORDER: readonly Feature[] = ['all-rows', 'backlog', 'archive', 'wiki', 'read-marks', 'tallies']

/** `assumed`: answered unsupported by the panel itself while the whole list was missing ({@link CompatProbe.lacks}). */
type Seen = 'unsupported' | 'assumed' | 'passed'

const sameList = (a: readonly string[], b: readonly string[]): boolean => a.length === b.length && a.every((item, n) => item === b[n])

/** Two reports that say the same; a banner drawn from the first need not be drawn (or read out) again. */
export function sameReport(a: CompatReport, b: CompatReport): boolean {
  return a.moai === b.moai && a.toolMissing === b.toolMissing && a.catalog.state === b.catalog.state &&
    sameList(a.catalog.missing, b.catalog.missing) && sameList(a.degraded, b.degraded)
}

/**
 * The compatibility banner already tells this refusal (install moai, update the catalog), so the board's own
 * banners need not tell it again. A start that failed naming the code is read as the refusal is. An
 * `unsupported-daemon` the catalog line does not stand for (the daemon as a whole too old, or nothing decided yet) is
 * still the board's to tell. `heard`: the read that ended so was handed to the probe (the detail is
 * not): the catalog line names only profiles it heard, while moai missing is the machine's and stands for any read.
 */
export function compatTells(report: CompatReport | null, outcome: Outcome | null, heard = true): boolean {
  const code = refusalCode(outcome)
  if (!report || code === undefined) return false
  if (code === 'tool-missing') return report.toolMissing
  return heard && code === 'unsupported-daemon' && report.catalog.state === 'missing'
}

/**
 * The banner told `outcome` under `before` ({@link compatTells}) and `after` says it was fixed: moai ran there since,
 * or the catalog lacks nothing now. Whatever told it again would tell what is no longer so (store.ts drops it).
 */
export function compatFixed(before: CompatReport | null, after: CompatReport, outcome: Outcome | null, heard = true): boolean {
  if (!compatTells(before, outcome, heard)) return false
  return refusalCode(outcome) === 'tool-missing' ? !after.toolMissing : after.catalog.state === 'ok'
}

export class CompatProbe {
  private readonly seen = new Map<string, Seen>()
  private toolMissing = false
  /** undefined: not seen yet. */
  private statusShape: { statuses: boolean; tallies: boolean } | undefined
  private readonly listeners = new Set<() => void>()

  /** `listener` is called after each thing handed over; the report may say what it said before. Returns the unsubscribe. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  // A listener is view code (main.ts puts the report in the store, which draws), told in the middle of a loader: one
  // that throws must not stop the read that handed the answer over (as client.ts notify() keeps its queue going).
  private told(): void {
    for (const listener of [...this.listeners]) {
      try {
        listener()
      } catch (error) {
        console.error(error)
      }
    }
  }

  /**
   * Not worth running there: an added profile other than the whole list, once the whole list was refused as
   * unsupported there (its other added profiles come with it; a daemon too old for the machine run refuses them all
   * the same), unless it has run there already — that is proof it is there. The caller answers such a request
   * `unsupported-daemon` itself.
   */
  lacks(profile: string): boolean {
    return profile !== PROFILE.showAll && ADDED.has(profile) && this.seen.get(profile) !== 'passed' && this.seen.get(PROFILE.showAll) === 'unsupported'
  }

  /** Hand over what a request to `profile` ended with. Information refusals and failures say nothing here. */
  observe(profile: string, outcome: Outcome): void {
    this.record(profile, outcome)
    this.told()
  }

  private record(profile: string, outcome: Outcome): void {
    switch (outcome.kind) {
      case 'refused':
      case 'not-started':
        // A start that failed naming a code is read as the refusal is. Only `unsupported-daemon` is about the catalog;
        // `binding-mismatch`, and a start that named nothing, say nothing here.
        if (outcome.code === 'tool-missing') this.toolMissing = true
        else if (outcome.code === 'unsupported-daemon') this.seen.set(profile, outcome.kind === 'refused' && outcome.assumed ? 'assumed' : 'unsupported')
        return
      case 'passed':
      case 'failed':
      case 'moai-error':
      case 'not-initialized':
      case 'unreadable':
        // It ran, so the catalog has the profile (a repository without `.moai/` is a board matter, not this), and moai
        // is there: once it is installed after a `tool-missing` refusal, the banner stops asking for it.
        this.seen.set(profile, 'passed')
        this.toolMissing = false
        return
      case 'cancelled':
        return
    }
  }

  /** The `status --json` output; anything that is not the counts object (the project list) is not read. */
  observeStatus(output: unknown): void {
    if (typeof output !== 'object' || output === null || Array.isArray(output)) return
    const record = output as Record<string, unknown>
    if (typeof record.counts !== 'object' || record.counts === null) return
    this.statusShape = {
      statuses: Array.isArray(record.statuses) && record.statuses.length > 0,
      tallies: Array.isArray(record.milestones) || Array.isArray(record.epics),
    }
    this.told()
  }

  report(): CompatReport {
    // A profile taken as missing with the whole list counts only while the whole list is: once it runs there, the
    // catalog was updated and nothing says the assumed ones are still missing (se-43oh).
    const listMissing = this.seen.get(PROFILE.showAll) === 'unsupported'
    const refused = [...this.seen].filter(([, seen]) => seen === 'unsupported' || (seen === 'assumed' && listMissing)).map(([id]) => id)
    const passed = [...this.seen.values()].filter((seen) => seen === 'passed').length
    // Refusals alone cannot tell a catalog that lacks the profiles from a daemon too old for the machine run at
    // all, so nothing is judged until a profile has run. A shipped profile refused as unsupported is the daemon as
    // a whole too old, and 12 missing profiles would say less than that.
    const wholeDaemon = refused.some((id) => BASELINE.has(id))
    const decided = passed > 0 && !wholeDaemon
    const missing = decided ? refused : []
    const catalogState: CompatReport['catalog']['state'] = !decided ? 'unknown' : missing.length > 0 ? 'missing' : 'ok'

    const old = this.statusShape !== undefined && (!this.statusShape.statuses || !this.statusShape.tallies)
    let moai: CompatReport['moai'] = 'unknown'
    if (old) moai = 'old'
    else if (this.statusShape !== undefined) moai = 'ok'

    const degraded = new Set<Feature>()
    for (const id of refused) {
      const feature = FEATURE_OF[id]
      if (feature !== undefined) degraded.add(feature)
    }
    if (this.statusShape !== undefined && !this.statusShape.statuses) degraded.add('status-order')
    if (this.statusShape !== undefined && !this.statusShape.tallies) degraded.add('tallies')

    return {
      moai,
      toolMissing: this.toolMissing,
      catalog: { state: catalogState, missing },
      degraded: ORDER.filter((feature) => degraded.has(feature)),
    }
  }
}
