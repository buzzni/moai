/**
 * The file tab's colour group of a name. The table is the server's (`/api/boot` → `file_groups`, built from the TUI's
 * `style::file_style`), so the page keeps no extension list of its own; this only matches the way Rust does.
 */
export const FILE_GROUPS = ['source', 'doc', 'data', 'script', 'binary'] as const
export type FileGroup = typeof FILE_GROUPS[number] | 'plain'
export type FileGroupTable = Partial<Record<typeof FILE_GROUPS[number], { ext: string[]; names: string[] }>>

const strings = (value: unknown): string[] => Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : []

/** The table out of a boot value: only the five known keys, only arrays of strings; anything else is dropped. */
export function fileGroupTable(raw: unknown): FileGroupTable {
  const out: FileGroupTable = {}
  if (!raw || typeof raw !== 'object') return out
  for (const group of FILE_GROUPS) {
    const entry = (raw as Record<string, unknown>)[group]
    if (entry && typeof entry === 'object') out[group] = { ext: strings((entry as Record<string, unknown>).ext), names: strings((entry as Record<string, unknown>).names) }
  }
  return out
}

/** Rust's `files::ext_of` (`Path::extension`): none for `.env`, empty for `a.`, the part after the last dot otherwise. */
export function extOf(name: string): string {
  const base = name.slice(name.lastIndexOf('/') + 1)
  const dot = base.lastIndexOf('.')
  if (dot <= 0) return ''
  return base.slice(dot + 1)
}

const lower = (text: string) => text.replace(/[A-Z]/g, c => c.toLowerCase())

/** The group of `name`: the first group holding its extension (ASCII-lowercased), else the first holding the exact name, else plain. */
export function fileGroup(name: string, table: FileGroupTable | null | undefined): FileGroup {
  if (!table) return 'plain'
  const ext = lower(extOf(name))
  if (ext) for (const group of FILE_GROUPS) if (table[group]?.ext.includes(ext)) return group
  const base = name.slice(name.lastIndexOf('/') + 1)
  for (const group of FILE_GROUPS) if (table[group]?.names.includes(base)) return group
  return 'plain'
}
