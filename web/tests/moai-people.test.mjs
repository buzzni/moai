// The page's copy of src/model.rs model::label: the same table as the Rust tests hold it to.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { panelModule } from './moai-panel-modules.mjs'

const { assigneeOf, label, useNaming } = await panelModule('data/people.ts')

test('label: the three namings, a name alone, an email alone, an empty name', () => {
  const table = [
    // name, email, naming, shown
    ['Dev', 'dev@example.com', 'full', 'Dev (dev@example.com)'],
    ['Dev', 'dev@example.com', 'name', 'Dev'],
    ['Dev', 'dev@example.com', 'email', 'dev@example.com'],
    ['Dev', undefined, 'email', 'Dev'],
    ['Dev', '   ', 'full', 'Dev'],
    ['', 'dev@example.com', 'name', 'dev@example.com'],
    ['  ', ' dev@example.com ', 'full', 'dev@example.com'],
  ]
  for (const [name, email, naming, shown] of table) assert.equal(label(name, email, naming), shown, `${name}|${email}|${naming}`)
})

test('the naming /api/boot gives is the one the screen uses; an unknown word is full', () => {
  const row = { assignee: 'Dev', assignee_email: 'dev@example.com' }
  try {
    useNaming('email')
    assert.equal(assigneeOf(row), 'dev@example.com')
    useNaming('nonsense')
    assert.equal(assigneeOf(row), 'Dev (dev@example.com)')
    assert.equal(assigneeOf({}), '', 'no assignee is nothing')
  } finally {
    useNaming('full')
  }
})
