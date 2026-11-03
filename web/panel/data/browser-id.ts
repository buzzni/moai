/** Coordinate the first stored id across tabs before any of them opens an event stream.
 * Without both a shared lock and writable storage, use the server's shared anonymous budget.
 */
export async function browserId(): Promise<string | null> {
  try {
    if (!navigator.locks) return null
    return await navigator.locks.request('moai-browser-id', () => {
      const key = 'moai-browser-id'
      const old = localStorage.getItem(key)
      if (old && /^[0-9a-f]{32}$/.test(old)) return old
      const id = Array.from(crypto.getRandomValues(new Uint8Array(16)), (n) => n.toString(16).padStart(2, '0')).join('')
      localStorage.setItem(key, id)
      return id
    })
  } catch { return null }
}
