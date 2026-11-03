const KEY = 'moai-browser-id'
const valid = (id: unknown): id is string => typeof id === 'string' && /^[0-9a-f]{32}$/.test(id)
const fresh = () => Array.from(crypto.getRandomValues(new Uint8Array(16)), (n) => n.toString(16).padStart(2, '0')).join('')

/** The legacy stored id, or a new one; throws when local storage cannot keep it. */
function stored(): string {
  const old = localStorage.getItem(KEY)
  if (valid(old)) return old
  const id = fresh()
  localStorage.setItem(KEY, id)
  return id
}

/** Get or create the origin's id in a single serialized readwrite transaction. */
function transactional(): Promise<string> {
  return new Promise((resolve, reject) => {
    const open = indexedDB.open(KEY, 1)
    let abandoned = false
    open.onupgradeneeded = () => { open.result.createObjectStore('identity') }
    open.onerror = () => reject(open.error)
    open.onblocked = () => { abandoned = true; reject(new Error('identity database blocked')) }
    open.onsuccess = () => {
      const db = open.result
      if (abandoned) { db.close(); return }
      try {
        const tx = db.transaction('identity', 'readwrite')
        const store = tx.objectStore('identity')
        let id: string
        const get = store.get(KEY)
        get.onsuccess = () => {
          if (valid(get.result)) { id = get.result; return }
          // Keep an existing identity when migrating. Local storage is optional once IndexedDB works.
          let legacy: string | null = null
          try { legacy = localStorage.getItem(KEY) } catch { /* storage denied */ }
          id = valid(legacy) ? legacy : fresh()
          store.put(id, KEY)
        }
        tx.oncomplete = () => { db.close(); resolve(id) }
        tx.onabort = () => { db.close(); reject(tx.error) }
        tx.onerror = () => { /* abort reports failure */ }
      } catch (error) { db.close(); reject(error) }
    }
  })
}

/** Origin-scoped stream budget identity, or null for the shared fallback when neither store works. */
export async function browserId(): Promise<string | null> {
  try { return await transactional() } catch {
    try { return stored() } catch { return null }
  }
}
