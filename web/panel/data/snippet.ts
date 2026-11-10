import { PRIORITY, type Client } from './client.js'
import type { SnippetRef, SnippetState } from '../view/markdown.js'

export const snippetKey = (ref: SnippetRef, page?: string): string => JSON.stringify([page ?? '', ref.path, ref.region ?? ''])
export async function loadSnippets(client: Client, refs: SnippetRef[], options: { page?: string, priority: number; words?: { missing: string; refused: string; tooLarge: string } }): Promise<Map<string, SnippetState>> {
  const entries = await Promise.all(refs.map(async ref => {
    const query: Record<string, string> = { path: ref.path }
    if (ref.region) query.region = ref.region
    if (options.page) query.page = options.page
    const result = await client.request({ path: '/api/snippet', query }, { priority: options.priority, share: false })
    if (result.kind === 'passed' && result.output && typeof result.output === 'object' && 'text' in result.output && typeof result.output.text === 'string') return [snippetKey(ref, options.page), { text: result.output.text }] as const
    const code = result.kind === 'moai-error' ? result.code : undefined
    const label = code === 'refused' ? options.words?.refused : code === 'too_large' ? options.words?.tooLarge : options.words?.missing
    const reason = result.kind === 'moai-error' ? result.message : ''
    return [snippetKey(ref, options.page), { error: [label ?? 'Unable to load snippet', reason].filter(Boolean).join(': ') }] as const
  }))
  return new Map<string, SnippetState>(entries)
}
export { PRIORITY }
