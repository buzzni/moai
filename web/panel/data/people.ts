/**
 * A person as the screen shows them: a name and an email, kept apart in the files and joined here, as `naming`
 * (`/api/boot`) says — `full` is `name (email)`, `name` the name, `email` the email. No DOM.
 */

export type Naming = 'full' | 'name' | 'email'

let naming: Naming = 'full'

/** Takes `/api/boot`'s `naming`; a word the page does not know leaves it `full`. */
export function useNaming(given: string): void {
  naming = given === 'name' || given === 'email' ? given : 'full'
}

/**
 * src/model.rs model::label 의 옮김 — 바꾸면 둘 다. A name left empty shows the email whatever the naming says, so the
 * line still says whose it is.
 */
export function label(name: string, email: string | undefined, how: Naming = naming): string {
  const mail = email?.trim() || undefined
  if (mail === undefined) return name
  if (name.trim() === '') return mail
  if (how === 'name') return name
  if (how === 'email') return mail
  return `${name} (${mail})`
}

/** A row's assignee as the screen shows it; '' when it has none. */
export function assigneeOf(row: { assignee?: unknown; assignee_email?: unknown }, how: Naming = naming): string {
  const name = typeof row.assignee === 'string' ? row.assignee : ''
  const email = typeof row.assignee_email === 'string' ? row.assignee_email : undefined
  return name === '' && email === undefined ? '' : label(name, email, how)
}
