const KEY = 'moai-browser-id'

/** The stored id, or a new one written in its place when none is stored. Throws when storage cannot be used. */
function stored(): { id: string; wrote: boolean } {
  const old = localStorage.getItem(KEY)
  if (old && /^[0-9a-f]{32}$/.test(old)) return { id: old, wrote: false }
  const id = Array.from(crypto.getRandomValues(new Uint8Array(16)), (n) => n.toString(16).padStart(2, '0')).join('')
  localStorage.setItem(KEY, id)
  return { id, wrote: true }
}

/**
 * The id the server budgets this browser's event streams by (src/web/ctl.rs), or null for the server's shared fallback
 * budget when storage cannot keep one. Tabs read and create it under an origin-wide Web Lock, so tabs opened at the
 * same first moment send one id. Browsers offer Web Locks only in a secure context (localhost, HTTPS): a page over
 * plain http on another address still keeps its own id, created without the lock — rather than sharing one budget
 * with every such browser — and only tabs opened at that same first moment may then hold different ids until reloaded.
 */
export async function browserId(): Promise<string | null> {
  try {
    if (!navigator.locks) return stored().id
    return await navigator.locks.request(KEY, async () => {
      const { id, wrote } = stored()
      // Released with the callback, the lock can reach a waiting tab before the write does: Firefox hands a page's
      // storage writes to the other processes at the end of its task. Holding it into the next task orders them.
      if (wrote) await new Promise((resolve) => setTimeout(resolve, 0))
      return id
    })
  } catch { return null }
}
