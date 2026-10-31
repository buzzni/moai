/** Suggestions are a small, nonmodal card in the visible tab. Text is always textContent. */
export interface Suggestion {
  id: string
  text: string
  then?: { verb: string; [key: string]: unknown } | null
  remaining_ms?: number
}

export interface ProposalWords {
  title: string
  accept: string
  decline: string
  previous: string
  next: string
}

/** Arguments the CLI takes in place; every other one is a `--flag`. */
const PLACED = new Set(['name', 'query', 'issue', 'slug', 'words', 'ids'])

/**
 * The follow-up as the CLI spells it — `view deferred off`, `read --all`, `sort title:desc,priority:desc` — so the
 * person sees what accepting does. Booleans, arrays and a cleared filter decide that and were dropped before.
 */
export function describeThen(then: NonNullable<Suggestion['then']>): string {
  const parts = [then.verb]
  for (const [key, value] of Object.entries(then)) {
    if (key === 'verb' || value === null || value === undefined) continue
    const flag = key === 'scope' ? '--in' : `--${key.replace(/_/g, '-')}`
    if (key === 'on') parts.push(value ? 'on' : 'off')
    else if (typeof value === 'boolean') { if (value) parts.push(flag) }
    else if (Array.isArray(value)) { if (value.length) parts.push(value.join(key === 'words' ? ',' : ' ')) }
    else if (PLACED.has(key)) parts.push(String(value))
    else if (!(key === 'scope' && value === 'all')) parts.push(`${flag} ${String(value)}`)
  }
  if (then.verb === 'filter' && (then.query === null || then.query === undefined)) parts.push('--clear')
  return parts.join(' ')
}

export function createProposalView(doc: Document, words: ProposalWords, answer: (entry: Suggestion, accepted: boolean) => void) {
  const rows: Suggestion[] = []
  const timers = new Map<string, ReturnType<typeof setTimeout>>()
  let at = 0
  const region = doc.createElement('section')
  region.className = 'm-proposal'
  region.setAttribute('role', 'region')
  region.setAttribute('aria-live', 'polite')
  region.hidden = true
  const heading = doc.createElement('strong')
  const message = doc.createElement('p')
  const action = doc.createElement('p')
  const controls = doc.createElement('div')
  const previous = doc.createElement('button')
  const next = doc.createElement('button')
  const accept = doc.createElement('button')
  const decline = doc.createElement('button')
  for (const button of [previous, next, accept, decline]) button.type = 'button'
  previous.className = next.className = 'm-btn m-btn-ghost m-btn-sm'
  accept.className = 'm-btn m-btn-primary m-btn-sm'
  decline.className = 'm-btn m-btn-outline m-btn-sm'
  previous.textContent = words.previous
  next.textContent = words.next
  accept.textContent = words.accept
  decline.textContent = words.decline
  controls.append(previous, next, accept, decline)
  region.append(heading, message, action, controls)
  doc.body.append(region)

  const current = () => rows[at] ?? null
  const draw = () => {
    region.hidden = rows.length === 0
    const row = current()
    if (!row) return
    heading.textContent = `» ${words.title} ${at + 1}/${rows.length}`
    message.textContent = row.text
    action.textContent = row.then ? `→ ${describeThen(row.then)}` : ''
    previous.hidden = next.hidden = rows.length < 2
  }
  const remove = (id: string) => {
    const index = rows.findIndex((row) => row.id === id)
    if (index < 0) return
    rows.splice(index, 1)
    const timer = timers.get(id)
    if (timer) clearTimeout(timer)
    timers.delete(id)
    at = Math.min(at, Math.max(0, rows.length - 1))
    draw()
  }
  previous.addEventListener('click', () => { at = (at + rows.length - 1) % rows.length; draw() })
  next.addEventListener('click', () => { at = (at + 1) % rows.length; draw() })
  accept.addEventListener('click', () => { const row = current(); if (row) answer(row, true) })
  decline.addEventListener('click', () => { const row = current(); if (row) answer(row, false) })
  return {
    el: region,
    current,
    count: () => rows.length,
    rows: () => rows.map((row) => ({ ...row })),
    show(entry: Suggestion) {
      if (rows.some((row) => row.id === entry.id)) return
      rows.push(entry)
      if (entry.remaining_ms && entry.remaining_ms > 0) timers.set(entry.id, setTimeout(() => remove(entry.id), entry.remaining_ms))
      draw()
    },
    remove,
    accept: () => { const row = current(); if (row) answer(row, true) },
    decline: () => { const row = current(); if (row) answer(row, false) },
    list: () => { if (rows.length) { at = (at + 1) % rows.length; draw() } },
    clear() { for (const row of [...rows]) remove(row.id) },
  }
}
