/**
 * The page's colour theme: `system` follows `prefers-color-scheme` as the page always did, `light` and `dark` are
 * the person's choice and win over the system. The choice is kept in the browser beside the other view settings
 * (data/memory.ts, key `moai.web`); `system` is no choice and stores no key. It is set as `<html data-theme>`, which
 * style/tokens.css already reads for both directions — and nothing is set for `system`, so the media query decides live.
 */
export type Theme = 'system' | 'light' | 'dark'

/** The order the button cycles through. */
export const THEMES: readonly Theme[] = ['system', 'light', 'dark']

export const nextTheme = (theme: Theme): Theme => THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length]!

/** A stored value to a theme; anything that is not a choice is `system`. */
export const normalizeTheme = (value: unknown): Theme => (value === 'light' || value === 'dark' ? value : 'system')

/** Puts the theme on the document element; `system` takes the attribute off so the system scheme is read again. */
export function applyTheme(doc: Document, theme: Theme): void {
  const root = doc.documentElement
  if (theme === 'system') delete root.dataset.theme
  else root.dataset.theme = theme
}

/**
 * The stored choice, read straight from the page's key before anything else runs, so the first paint is already the
 * chosen theme. Whatever throws (site data blocked) or does not parse is `system`.
 */
export function storedTheme(key: string, backend: () => { getItem(key: string): string | null } | null | undefined): Theme {
  try {
    const text = backend()?.getItem(key)
    return text ? normalizeTheme((JSON.parse(text) as { theme?: unknown } | null)?.theme) : 'system'
  } catch {
    return 'system'
  }
}
