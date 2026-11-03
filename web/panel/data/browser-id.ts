const KEY = 'moai-browser-id'
/**
 * How long the page waits for IndexedDB before it falls back. An open or transaction that never answers (a stuck
 * store, another tab frozen mid-transaction) must not keep the tab off its event stream for its whole life.
 */
export const IDENTITY_WAIT_MS = 5_000
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

/** Get or create the origin's id in a single serialized readwrite transaction; rejects past `waitMs`. */
function transactional(waitMs: number): Promise<string> {
  return new Promise((resolve, reject) => {
    // Opened before the timer: a store that throws here (denied, absent) rejects without leaving a timer behind.
    const open = indexedDB.open(KEY, 1)
    let abandoned = false
    let transaction: IDBTransaction | undefined
    const timer = setTimeout(() => fail(new Error('identity database did not answer')), waitMs)
    function fail(error: unknown) {
      abandoned = true
      clearTimeout(timer)
      try { transaction?.abort() } catch { /* already finished */ }
      reject(error)
    }
    open.onupgradeneeded = () => { open.result.createObjectStore('identity') }
    open.onerror = () => fail(open.error)
    open.onblocked = () => fail(new Error('identity database blocked'))
    open.onsuccess = () => {
      const db = open.result
      if (abandoned) { db.close(); return }
      try {
        const tx = db.transaction('identity', 'readwrite')
        transaction = tx
        const store = tx.objectStore('identity')
        let id: string
        const get = store.get(KEY)
        get.onsuccess = () => {
          if (abandoned) return
          if (valid(get.result)) { id = get.result; return }
          // Keep an existing identity when migrating. Local storage is optional once IndexedDB works.
          let legacy: string | null = null
          try { legacy = localStorage.getItem(KEY) } catch { /* storage denied */ }
          id = valid(legacy) ? legacy : fresh()
          store.put(id, KEY)
        }
        tx.oncomplete = () => { db.close(); clearTimeout(timer); resolve(id) }
        tx.onabort = () => { db.close(); fail(tx.error) }
        tx.onerror = () => { /* abort reports failure */ }
      } catch (error) { db.close(); fail(error) }
    }
  })
}

/** Origin-scoped stream budget identity, or null for the shared fallback when neither store works. */
export async function browserId(waitMs = IDENTITY_WAIT_MS): Promise<string | null> {
  let id: string
  try { id = await transactional(waitMs) } catch {
    try { return stored() } catch { return null }
  }
  // Mirror it into the fallback store: a later tab whose IndexedDB fails must land on this id, not open a second
  // three-stream budget in the same browser's connection pool.
  try { if (localStorage.getItem(KEY) !== id) localStorage.setItem(KEY, id) } catch { /* storage denied */ }
  return id
}
