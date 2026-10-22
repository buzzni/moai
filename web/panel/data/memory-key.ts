/**
 * The page's key in localStorage. The origin is `moai web`'s host:port, which every project the page reads shares
 * (`?project=`): what is kept here is the person's, and what is a project's own goes under data/memory.ts queryKeyOf.
 *
 * A module of its own so the page's first script (early.ts, built to theme.js) can read the key without bundling
 * data/memory.ts, whose imports would pull the store into a script that has to stay a few hundred bytes.
 */
export const MEMORY_KEY = 'moai.web'
