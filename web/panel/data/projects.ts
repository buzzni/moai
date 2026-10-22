/**
 * The registered projects the page can switch between (moai-o14f), drawn as the TUI's header draws them (src/tui/draw.rs
 * `numbered_projects`, `with_projects`): `<1> name` … `<9> name` in the order they were registered, the tenth and on
 * without a number. The TUI's `<0>` is its all-projects layer, which the page does not have, so it is left out.
 *
 * - The list is `/api/projects` (`moai project ls --json`: `{config, projects:[{name, path, …}], problems}`).
 * - The project the page shows is its address's `?project=<path>` (data/client.ts projectOf), else the server's own
 *   tracker — the home. A home that is not registered stands first, unnumbered, named by its directory.
 * - Switching is a new page: `/` for the home, `/?project=<path>` for any other ({@link projectHref}). The server reads
 *   only a registered path (403 `not_registered`), so the page sends back the `path` it was given as it is.
 *
 * Nothing is registered or unregistered here: the TUI's `SPC p a`/`SPC p d` write the user config. No DOM here.
 */

/** One registered project as `project ls --json` prints it; what the page reads of it. */
export interface Project {
  name: string
  path: string
}

/** One entry of the header's project column. */
export interface ProjectEntry {
  name: string
  path: string
  /** `1`–`9`, the digit that jumps to it; null past the ninth and for an unregistered home. */
  number: number | null
  /** The server's own tracker. */
  home: boolean
  /** The project the page shows now. */
  current: boolean
}

/** The digits a project can carry (the TUI's `keys::NUMBERED`): one key each, never two. */
export const NUMBERED = 9

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** `/api/projects`' answer to its projects; an answer of any other shape is none. */
export function projectsFrom(output: unknown): Project[] {
  if (!isRecord(output) || !Array.isArray(output.projects)) return []
  return output.projects.flatMap((one) =>
    isRecord(one) && typeof one.path === 'string' && one.path !== ''
      ? [{ name: oneLine(typeof one.name === 'string' && one.name !== '' ? one.name : baseName(one.path)), path: one.path }]
      : [])
}

/** A directory name written by someone else may hold a line break or a tab: drawn on one line (the TUI's `text::one_line`). */
const oneLine = (text: string): string => text.replace(/[\u0000-\u001f\u007f]+/g, ' ')

/** The last part of a path. */
export const baseName = (path: string): string => trimmed(path).split('/').at(-1) || path

const trimmed = (path: string): string => (path.length > 1 ? path.replace(/\/+$/, '') : path)

/** Two paths name the same directory, a trailing slash aside. */
export const samePath = (a: string | null | undefined, b: string | null | undefined): boolean => !!a && !!b && trimmed(a) === trimmed(b)

/**
 * The header's entries: an unregistered home first, then the registered projects numbered `1`–`9` in their order.
 * `current` is the project the page shows: `?project=` when it names one, else the home.
 */
export function headerProjects(registered: readonly Project[], home: { root: string; name: string } | null, shown: string | null): ProjectEntry[] {
  const here = shown ?? home?.root ?? null
  const entries: ProjectEntry[] = []
  if (home && !registered.some((one) => samePath(one.path, home.root))) {
    entries.push({ name: oneLine(home.name || baseName(home.root)), path: home.root, number: null, home: true, current: samePath(home.root, here) })
  }
  registered.forEach((one, n) => {
    entries.push({ name: one.name, path: one.path, number: n < NUMBERED ? n + 1 : null, home: samePath(one.path, home?.root), current: samePath(one.path, here) })
  })
  return entries
}

/** The names of the numbered entries, by digit: the key table labels `1`–`9` and `SPC p 1`–`9` with them. */
export const numberedNames = (entries: readonly ProjectEntry[]): string[] =>
  entries.filter((entry) => entry.number !== null).sort((a, b) => a.number! - b.number!).map((entry) => entry.name)

/** The entry a digit jumps to; undefined past the projects that exist. */
export const entryOf = (entries: readonly ProjectEntry[], number: number): ProjectEntry | undefined => entries.find((entry) => entry.number === number)

/** The page's address for a project: `/` for the home (registered or not), `/?project=<path>` for any other. */
export function projectHref(path: string, homeRoot: string | null): string {
  return samePath(path, homeRoot) ? '/' : `/?project=${encodeURIComponent(path)}`
}
