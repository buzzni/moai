import assert from 'node:assert/strict'
import { test } from 'node:test'
import { panelModule } from './moai-panel-modules.mjs'
const { createClient, withProject, PRIORITY } = await panelModule('data/client.ts')
const { loadSnippets, snippetKey } = await panelModule('data/snippet.ts')

test('snippet requests preserve the written path, page, region and project', async () => {
  const seen = []
  const fetch = async url => { seen.push(new URL(url, 'http://localhost')); return { status: 200, text: async () => JSON.stringify({exit:0,out:{text:'hello'}}) } }
  const client = createClient(withProject(fetch, '/other'))
  const ref = { path:'../../src/x.rs', region:'r' }
  const values = await loadSnippets(client, [ref], {page:'docs/guide/a.md', priority: PRIORITY.wiki})
  assert.deepEqual(values.get(snippetKey(ref, 'docs/guide/a.md')), {text:'hello'})
  assert.equal(seen[0].searchParams.get('path'), '../../src/x.rs')
  assert.equal(seen[0].searchParams.get('region'), 'r')
  assert.equal(seen[0].searchParams.get('page'), 'docs/guide/a.md')
  assert.equal(seen[0].searchParams.get('project'), '/other')
})

test('snippet server refusal remains a text error', async () => {
  const client = createClient(async () => ({status:403, text:async () => JSON.stringify({error:'denied',code:'refused'})}))
  const ref = {path:'.env'}
  const values = await loadSnippets(client, [ref], {priority:0, words:{refused:'Refused', missing:'Missing', tooLarge:'Too large'}})
  assert.deepEqual(values.get(snippetKey(ref)), {error:'Refused: denied'})
})
