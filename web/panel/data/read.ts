/**
 * Which rows are unread. No DOM here, and no writes: the page only reads (`moai web` serves GETs alone), so marking
 * rows read is `moai read` in a terminal or the explorer.
 *
 * Judgement (se-ehdi.bzp). The page computes nothing. "Near me" (assigned to me, their `.children`, members
 * of my epics; milestones not counted) and `updated_at > seen` are decided by the CLI, which prints
 * `"unread": true` on `moai show --json` rows and on `moai show <id> --json`. The key is there only when true and
 * absent otherwise; with the person unknown it is absent everywhere. So when no row carries it, NEW is simply off.
 */
import type { Row } from './index.js'

export const isUnread = (row: Row): boolean => row.unread === true && row.archived !== true

export const unreadIds = (rows: readonly Row[]): string[] => rows.filter(isUnread).map((row) => row.id)
