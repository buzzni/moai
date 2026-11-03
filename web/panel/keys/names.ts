/** Shared translation between router notation, ctl wire names and visible key labels. */
const display: Readonly<Record<string, string>> = { Up: "↑", Down: "↓", Left: "←", Right: "→", Backspace: "Bksp", PageUp: "PgUp", PageDown: "PgDn" }
const aliases: Readonly<Record<string, string>> = { Bksp: "Backspace", PgUp: "PageUp", PgDn: "PageDown" }

export function wireToken(token: string): string {
  return token.replace(/^(?:(?:C|A|S)-)+/, (mods) => mods.replace(/C-/g, "Ctrl-").replace(/A-/g, "Alt-").replace(/S-/g, "Shift-")).replace(/Backspace$/, "Bksp")
}
export function routerTokens(seq: string): string[] {
  return seq.split(/\s+/).filter(Boolean).flatMap((token) => {
    if (/^([a-z])\1$/i.test(token)) return [token[0]!, token[1]!]
    const m = /^((?:(?:Ctrl|Alt|Shift)-)*)(.+)$/.exec(token)!
    let mods = m[1]!.replace(/Ctrl-/g, "C-").replace(/Alt-/g, "A-").replace(/Shift-/g, "S-")
    let name = m[2]!
    if (name === "BackTab") { name = "Tab"; if (!mods.includes("S-")) mods += "S-" }
    return [mods + (aliases[name] ?? name)]
  })
}
export function keyLabel(notation: string): string {
  if (notation === "g g") return "gg"
  return notation.split(" ").filter(Boolean).map((token) => {
    const m = /^((?:[CAS]-)*)(.+)$/.exec(token)!
    const mods = m[1]!.replace(/C-/g, "Ctrl-").replace(/A-/g, "Alt-").replace(/S-/g, "Shift-")
    return mods + (display[m[2]!] ?? m[2]!)
  }).join(" ")
}
