// Moai panel keys: the key table as data and the key router (docs/moai-panel/keybindings.md §2, §3).
// panel/keys/*.ts are standalone ES modules; Node strips the types.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { JSDOM } from 'jsdom'

const panel = new URL('../panel/', import.meta.url)
const table = await import(new URL('keys/table.ts', panel))
const routerModule = await import(new URL('keys/router.ts', panel))
const { KEY_TABLE, MENU_CONTROLS, LAYERS, menuLayer, helpSections, keyLabel, bindingKeys, tokens, say } = table
const { TABLE, copyFor } = await import(new URL('i18n/index.ts', panel))
const { createKeyRouter, attachKeyRouter, keyToken, isEditableTarget, detectPlatform } = routerModule

const browse = (extra = {}) => ({ mode: 'browse', tab: 'list', focus: 'list', detail: true, detailAt: 'bottom', ...extra })

function setup ({ platform = 'other', ctx = browse() } = {}) {
  const sent = []
  const changes = []
  const state = { ctx }
  const router = createKeyRouter({
    platform,
    getContext: () => state.ctx,
    dispatch: (action) => sent.push(action),
    onChange: (s) => changes.push(s)
  })
  const key = (k, init = {}) => router.handle({ key: k, ...init })
  const ids = () => sent.map((a) => a.id)
  return { router, sent, changes, state, key, ids }
}

const allKeys = (bindings, platform) => bindings.flatMap((b) => bindingKeys(b, platform))

// ------------------------------------------------------------------ table integrity

test('table: no key sequence is bound twice, or is both an action and a prefix, in any mode', () => {
  for (const platform of ['mac', 'other']) {
    for (const mode of ['browse', 'wiki', 'prompt', 'sorter']) {
      const bindings = [...KEY_TABLE[mode], ...(mode === 'browse' || mode === 'wiki' ? KEY_TABLE.menu.filter((b) => !b.modes || b.modes.includes(mode)) : [])]
      const keys = allKeys(bindings, platform)
      const dupes = keys.filter((k, i) => keys.indexOf(k) !== i)
      assert.deepEqual(dupes, [], `${mode}/${platform}: duplicate keys`)
      for (const k of keys) {
        const longer = keys.find((other) => other !== k && other.startsWith(`${k} `))
        assert.equal(longer, undefined, `${mode}/${platform}: ${k} is an action and the prefix of ${longer}`)
      }
    }
  }
})

test('table: every binding has an action id, words in all four languages and a key; menu bindings start with SPC', () => {
  const ctx = { mode: 'browse', columns: ['todo'] }
  for (const [mode, bindings] of Object.entries(KEY_TABLE)) {
    for (const b of bindings) {
      assert.match(b.action, /^[a-zA-Z.]+$/, `${mode}: action id`)
      for (const [lang, words] of Object.entries(TABLE)) {
        for (const label of [b.label, b.helpLabel].filter(Boolean)) {
          const said = say(label, ctx, words)
          assert.ok(typeof said === 'string' && said.length > 0, `${mode}/${b.action}: ${lang} label`)
        }
      }
      assert.ok(allKeys([b], 'mac').length > 0, `${mode}/${b.action}: at least one key`)
      if (mode === 'menu') {
        for (const k of b.keys) assert.equal(tokens(k)[0], 'SPC', `${k} is under SPC`)
        assert.ok(b.modes?.length, `${b.action}: menu binding names its modes`)
      }
    }
  }
  for (const c of MENU_CONTROLS) for (const words of Object.values(TABLE)) assert.ok(say(c.label, ctx, words), c.action)
  // Every menu layer has a title.
  const prefixes = new Set(KEY_TABLE.menu.flatMap((b) => b.keys.map((k) => tokens(k).slice(0, -1).join(' '))).filter((p) => p !== 'SPC'))
  for (const p of prefixes) assert.ok(LAYERS.some((l) => l.prefix === p), `layer ${p} has a title`)
  for (const words of Object.values(TABLE)) for (const l of LAYERS) assert.ok(words.layers[l.id], `layer ${l.prefix} has words`)
})

test('table: the menu and help speak the words they are given; Korean when none', () => {
  const en = TABLE.en
  const root = menuLayer(['SPC'], browse(), en)
  assert.equal(root.title, 'Menu')
  assert.equal(root.items.find((i) => i.key === 'v').label, '+View')
  const view = Object.fromEntries(menuLayer('SPC v', browse({ hidden: { deferred: true } }), en).items.map((i) => [i.key, i]))
  assert.equal(view.l.label, 'Deferred')
  assert.equal(view.l.state.label, '[hidden]')
  const sort = Object.fromEntries(menuLayer('SPC s', browse(), TABLE.ja).items.map((i) => [i.key, i]))
  assert.equal(sort.P.label, '優先度 (追加)')
  const help = helpSections('browse', browse(), 'other', TABLE.zh)
  assert.equal(help[0].title, '移动')
  assert.equal(help[0].rows[0].label, '下移一行')
  assert.equal(menuLayer(['SPC'], browse()).title, '메뉴')
})

test('table: C-d / C-u say the cursor stays in browse, not in the wiki, whose list moves its cursor (9w2 #12)', () => {
  const half = (mode) => helpSections(mode, { mode }, 'other').flatMap((s) => s.rows).find((r) => r.action === 'move.halfDown').label
  assert.match(half('browse'), /커서 그대로/)
  assert.doesNotMatch(half('wiki'), /커서/)
  // Every language tells the two apart: the wiki's row is not the browse row's words.
  for (const [lang, words] of Object.entries(TABLE)) {
    const said = (mode) => helpSections(mode, { mode }, 'other', words).flatMap((s) => s.rows).find((r) => r.action === 'move.halfDown').label
    assert.notEqual(said('wiki'), said('browse'), lang)
    assert.ok(said('browse').startsWith(said('wiki')), `${lang}: the browse row adds that the cursor stays`)
  }
})

test('table: 제외 keys are absent (0-9, SPC q, SPC g s, SPC v w, SPC c w, SPC o t/m, SPC p, Ctrl-C)', () => {
  const everything = Object.values(KEY_TABLE).flatMap((bs) => allKeys(bs, 'mac'))
  for (const n of '0123456789') assert.ok(!KEY_TABLE.browse.some((b) => bindingKeys(b, 'mac').includes(n)), `browse ${n}`)
  for (const gone of ['SPC q', 'SPC g s', 'SPC v w', 'SPC c w', 'SPC o t', 'SPC o m', 'SPC p a', 'SPC p d', 'C-c']) {
    assert.ok(!everything.includes(gone), `${gone} is excluded`)
  }
})

test('table: no Ctrl-w anywhere (it closes the browser tab); SPC w and Alt-Backspace everywhere', () => {
  const everything = Object.values(KEY_TABLE).flat().flatMap((b) => [...bindingKeys(b, 'mac'), ...bindingKeys(b, 'other')])
  assert.equal(everything.filter((key) => key.startsWith('C-w')).length, 0)
  assert.ok(KEY_TABLE.menu.some((b) => b.action === 'focus.next' && b.keys.includes('SPC w w')))
  for (const side of ['W', 'h', 'j', 'k', 'l']) assert.ok(KEY_TABLE.menu.some((b) => b.keys.includes(`SPC w ${side}`)), `SPC w ${side}`)
  const word = KEY_TABLE.prompt.find((b) => b.action === 'prompt.deleteWord')
  assert.deepEqual(bindingKeys(word, 'other'), ['A-Backspace'])
  assert.deepEqual(bindingKeys(word, 'mac'), ['A-Backspace'])
  // 변경: SPC o d toggles left/bottom. SPC n (add) is gone: the panel reads only (se-24ey.oxd).
  assert.equal(KEY_TABLE.menu.find((b) => b.keys.includes('SPC n')), undefined)
  assert.equal(KEY_TABLE.menu.find((b) => b.keys.includes('SPC o d')).action, 'option.detailAt')
})

test('table: 051 bindings — motion, enter/leave, tabs, search, help, prompt, sorter, wiki', () => {
  const find = (mode, key) => KEY_TABLE[mode].find((b) => bindingKeys(b, 'mac').includes(key))?.action
  const expect = {
    browse: { j: 'move.down', Down: 'move.down', k: 'move.up', Up: 'move.up', 'g g': 'move.top', Home: 'move.top', G: 'move.bottom', End: 'move.bottom', 'C-d': 'move.halfDown', 'C-u': 'move.halfUp', 'C-f': 'move.pageDown', PageDown: 'move.pageDown', 'C-b': 'move.pageUp', PageUp: 'move.pageUp', Enter: 'enter', Backspace: 'leave', l: 'expand', Right: 'expand', h: 'collapse', Left: 'collapse', Tab: 'expandAll', '/': 'search.open', Esc: 'clear', '?': 'help' },
    menu: { 'SPC /': 'search.open', 'SPC f': 'filter.open', 'SPC g l': 'tab.list', 'SPC g b': 'tab.board', 'SPC g w': 'tab.wiki', 'SPC ?': 'help', 'SPC v r': 'view.raw', 'SPC s e': 'sort.edit' },
    prompt: { Enter: 'prompt.apply', Esc: 'prompt.cancel', Tab: 'prompt.next', 'S-Tab': 'prompt.prev', Up: 'prompt.up', Down: 'prompt.down', 'C-u': 'prompt.clear' },
    sorter: { j: 'sorter.down', k: 'sorter.up', J: 'sorter.lower', K: 'sorter.raise', SPC: 'sorter.toggle', d: 'sorter.flip', Enter: 'sorter.apply', D: 'sorter.default', Esc: 'sorter.cancel' },
    wiki: { j: 'move.down', 'g g': 'move.top', Enter: 'wiki.enter', Backspace: 'wiki.back', h: 'wiki.back', Left: 'wiki.back', Tab: 'wiki.nextLink', 'S-Tab': 'wiki.prevLink', '/': 'wiki.search', Esc: 'wiki.clear' }
  }
  for (const [mode, keys] of Object.entries(expect)) {
    for (const [key, action] of Object.entries(keys)) assert.equal(find(mode, key), action, `${mode} ${key}`)
  }
})

// ------------------------------------------------------------------ router: common rules

test('keyToken: named keys, modifiers, shift folded into characters, meta and lone modifiers ignored', () => {
  assert.equal(keyToken({ key: ' ' }), 'SPC')
  assert.equal(keyToken({ key: 'ArrowDown' }), 'Down')
  assert.equal(keyToken({ key: 'Escape' }), 'Esc')
  assert.equal(keyToken({ key: 'G', shiftKey: true }), 'G')
  assert.equal(keyToken({ key: 'd', ctrlKey: true }), 'C-d')
  assert.equal(keyToken({ key: 'Tab', shiftKey: true }), 'S-Tab')
  assert.equal(keyToken({ key: 'Backspace', altKey: true }), 'A-Backspace')
  assert.equal(keyToken({ key: 'k', metaKey: true }), null)
  assert.equal(keyToken({ key: 'Shift', shiftKey: true }), null)
})

test('keyToken: Shift still down from a capital does not turn Space, Enter or Esc into other keys', () => {
  assert.equal(keyToken({ key: ' ', shiftKey: true }), 'SPC')
  assert.equal(keyToken({ key: 'Enter', shiftKey: true }), 'Enter')
  assert.equal(keyToken({ key: 'Escape', shiftKey: true }), 'Esc')
  assert.equal(keyToken({ key: 'Backspace', shiftKey: true }), 'Backspace')
  assert.equal(keyToken({ key: 'Tab', shiftKey: true }), 'S-Tab')
  const { key, router } = setup()
  key('G', { shiftKey: true })
  key(' ', { shiftKey: true })
  assert.deepEqual(router.state().menu, ['SPC'], 'G then Space with Shift still held opens the menu')
})

test('router: a click on an item of a menu that a change of mode dropped runs nothing', () => {
  let mode = 'wiki'
  const sent = []
  const router = createKeyRouter({
    platform: 'other',
    getContext: () => ({ ...browse(), mode, tab: mode === 'wiki' ? 'wiki' : 'list' }),
    dispatch: (a) => sent.push(a.id),
  })
  router.press('SPC')
  router.press('v')
  assert.deepEqual(router.state().menu, ['SPC', 'v'])
  mode = 'browse' // a click on the List tab, with the overlay still up
  assert.deepEqual(router.press('r'), { handled: false, preventDefault: false })
  assert.equal(router.state().menu, null)
  assert.deepEqual(sent, [], '`r` is not run as a key of the new mode')
})

test('router: an editable target turns browse keys off; prompt mode keys still apply', () => {
  const { key, ids, router, state } = setup()
  assert.deepEqual(key('j', { editable: true }), { handled: false, preventDefault: false })
  assert.deepEqual(key('/', { editable: true }), { handled: false, preventDefault: false })
  assert.deepEqual(ids(), [])
  // The target object is checked when no flag is passed.
  const dom = new JSDOM('<!doctype html><input id="i"><div contenteditable><span id="s"></span></div><button id="b"></button>')
  const d = dom.window.document
  assert.equal(key('j', { target: d.getElementById('i') }).handled, false)
  assert.equal(key('j', { target: d.getElementById('s') }).handled, false)
  assert.equal(key('j', { target: d.getElementById('b') }).handled, true)
  // A pending sequence is dropped when focus lands in an input.
  key('g')
  assert.deepEqual(router.state().pending, ['g'])
  key('x', { editable: true })
  assert.deepEqual(router.state().pending, [])
  state.ctx = { mode: 'prompt' }
  assert.equal(key('Enter', { editable: true }).action.id, 'prompt.apply')
  assert.equal(key('Tab', { editable: true }).preventDefault, true)
  assert.equal(key('j', { editable: true }).handled, false, 'letters type into the input')
})

test('router: IME composition handles nothing and leaves the buffer alone', () => {
  const { key, ids, router, state } = setup()
  key('g')
  assert.deepEqual(key('g', { isComposing: true }), { handled: false, preventDefault: false })
  assert.deepEqual(key('ㅎ', { keyCode: 229 }), { handled: false, preventDefault: false })
  assert.deepEqual(router.state().pending, ['g'], 'composition does not touch the pending sequence')
  state.ctx = { mode: 'prompt' }
  assert.equal(key('Enter', { isComposing: true }).handled, false, 'Enter that ends a composition does not apply')
  assert.deepEqual(ids(), [])
})

test('router: Space, Backspace and / get preventDefault in browse even when they do nothing; Tab only when used', () => {
  const { key } = setup({ ctx: browse({ tab: 'board', focus: 'detail' }) })
  for (const [k, init] of [['Backspace', {}], ['/', {}], [' ', {}]]) {
    assert.equal(key(k, init).preventDefault, true, `${k} ${JSON.stringify(init)}`)
  }
  key('Escape')
  // se-h5c7: Tab that runs nothing moves the focus as in any page (it was swallowed everywhere in browse).
  assert.deepEqual(key('Tab'), { handled: false, preventDefault: false }, 'expandAll is unavailable on the board')
  assert.deepEqual(key('Tab', { shiftKey: true }), { handled: false, preventDefault: false })
  assert.deepEqual(key('x'), { handled: false, preventDefault: false }, 'other unbound keys pass through')
})

test('router: an action whose handler says it did not use the key (false) leaves the key to the browser', () => {
  const sent = []
  const router = createKeyRouter({ platform: 'other', getContext: () => ({ mode: 'prompt' }), dispatch: (a) => (sent.push(a.id), false) })
  const tab = router.handle({ key: 'Tab', editable: true })
  assert.deepEqual([tab.handled, tab.preventDefault, tab.action?.id], [false, false, 'prompt.next'])
  assert.equal(router.handle({ key: 'ArrowUp', editable: true }).preventDefault, false)
  assert.deepEqual(sent, ['prompt.next', 'prompt.up'], 'the action still ran')
  // A handler that returns anything else used it.
  const used = createKeyRouter({ platform: 'other', getContext: () => ({ mode: 'prompt' }), dispatch: () => 3 })
  assert.equal(used.handle({ key: 'Tab', editable: true }).preventDefault, true)
})

test('router: settle() drops a menu or sequence held in a mode the panel has left', () => {
  const { router, key, state } = setup()
  key(' ')
  assert.deepEqual(router.state().menu, ['SPC'])
  router.settle()
  assert.deepEqual(router.state().menu, ['SPC'], 'same mode: kept')
  state.ctx = { ...state.ctx, mode: 'prompt' }
  router.settle()
  assert.equal(router.state().menu, null, 'focus went into an input: the menu closes')
  state.ctx = browse()
  key('g')
  state.ctx = browse({ mode: 'wiki', tab: 'wiki' })
  router.settle()
  assert.deepEqual(router.state().pending, [])
})

test('router: motion and the other direct keys dispatch their action ids', () => {
  const { key, sent } = setup()
  key('j'); key('ArrowUp'); key('G', { shiftKey: true }); key('End'); key('d', { ctrlKey: true }); key('PageUp')
  key('Enter'); key('Backspace'); key('l'); key('ArrowLeft'); key('Tab'); key('/'); key('Escape'); key('?')
  assert.deepEqual(sent.map((a) => a.id), ['move.down', 'move.up', 'move.bottom', 'move.bottom', 'move.halfDown', 'move.pageUp', 'enter', 'leave', 'expand', 'collapse', 'expandAll', 'search.open', 'clear', 'help'])
  assert.deepEqual(sent[0], { id: 'move.down', keys: ['j'], mode: 'browse', fromMenu: false })
})

test('router: gg goes to the top; g then another key drops both (keys.rs Chord::feed)', () => {
  const { key, ids, router, changes } = setup()
  assert.deepEqual(key('g'), { handled: true, preventDefault: true })
  assert.deepEqual(router.state(), { pending: ['g'], menu: null })
  key('Shift', { shiftKey: true })
  assert.deepEqual(router.state().pending, ['g'], 'a lone modifier keydown keeps the buffer')
  assert.equal(key('g').action.id, 'move.top')
  assert.deepEqual(router.state().pending, [])
  key('g')
  assert.deepEqual(key('x'), { handled: true, preventDefault: true }, 'gx is swallowed')
  key('g')
  assert.equal(key('j').action, undefined, 'gj does not move down')
  assert.deepEqual(router.state().pending, [])
  assert.equal(key('g').handled, true, 'the dropped g does not join a new one')
  assert.equal(key('g').action.id, 'move.top')
  assert.deepEqual(ids(), ['move.top', 'move.top'])
  assert.deepEqual(changes.at(-1), { pending: [], menu: null })
})

test('router: Ctrl-w is the browser\'s on every platform, SPC w moves the focus', () => {
  for (const platform of ['mac', 'other']) {
    const r = setup({ platform })
    assert.deepEqual(r.key('w', { ctrlKey: true }), { handled: false, preventDefault: false }, platform)
    assert.equal(r.key('w').handled, false)
    assert.deepEqual(r.ids(), [])
  }
  for (const r of [setup({ platform: 'other' }), setup({ platform: 'mac' })]) {
    r.key(' '); r.key('w')
    assert.equal(r.key('w').action.id, 'focus.next')
    r.key(' '); r.key('w')
    assert.equal(r.key('W', { shiftKey: true }).action.id, 'focus.prev')
  }
})

// ------------------------------------------------------------------ router: SPC menu

test('menu: SPC opens, layers go down, Backspace goes up, Esc and SPC close', () => {
  const { key, router, ids } = setup()
  assert.deepEqual(key(' '), { handled: true, preventDefault: true })
  assert.deepEqual(router.state(), { pending: [], menu: ['SPC'] })
  key('v')
  assert.deepEqual(router.state().menu, ['SPC', 'v'])
  key('Backspace')
  assert.deepEqual(router.state().menu, ['SPC'], 'Backspace goes up one layer')
  key('Backspace')
  assert.equal(router.state().menu, null, 'Backspace at the root closes')
  key(' '); key('s')
  key('Escape')
  assert.equal(router.state().menu, null, 'Esc closes')
  key(' '); key('o')
  key(' ')
  assert.equal(router.state().menu, null, 'SPC closes')
  assert.deepEqual(ids(), [], 'closing keys run nothing (Esc does not clear the search)')
})

test('menu: unknown keys are ignored and swallowed; action items close, toggle items keep it open', () => {
  const { key, router, sent } = setup()
  key(' ')
  assert.deepEqual(key('z'), { handled: true, preventDefault: true })
  assert.deepEqual(key('Tab'), { handled: true, preventDefault: true })
  assert.deepEqual(router.state().menu, ['SPC'], 'unknown key leaves the menu open')
  key('v'); key('l')
  assert.deepEqual(router.state().menu, ['SPC', 'v'], 'toggle keeps the layer open')
  key('d')
  key('a')
  assert.equal(router.state().menu, null, 'SPC v a (show all) is an action and closes')
  key(' '); key('s'); key('P', { shiftKey: true })
  assert.deepEqual(router.state().menu, ['SPC', 's'])
  key('e')
  assert.equal(router.state().menu, null)
  key(' '); key('g')
  assert.equal(key('b').action.id, 'tab.board')
  assert.equal(router.state().menu, null)
  // SPC n added a task once; the panel reads only now (se-24ey.oxd), so it is an unknown key there.
  key(' ')
  assert.deepEqual(key('n'), { handled: true, preventDefault: true })
  assert.deepEqual(router.state().menu, ['SPC'])
  key('Escape')
  assert.deepEqual(sent.map((a) => [a.id, a.args, a.fromMenu]), [
    ['view.deferred', undefined, true],
    ['view.detail', undefined, true],
    ['view.all', undefined, true],
    ['sort.add', { field: 'priority' }, true],
    ['sort.edit', undefined, true],
    ['tab.board', undefined, true],
  ])
  assert.deepEqual(sent[0].keys, ['SPC', 'v', 'l'])
})

test('menu: a motion key the layer does not carry closes the menu and moves; layer letters win', () => {
  const { key, router, ids } = setup()
  key(' ')
  assert.equal(key('j').action.id, 'move.down')
  assert.equal(router.state().menu, null)
  key(' '); key('v')
  assert.equal(key('G', { shiftKey: true }).action.id, 'move.bottom')
  key(' ')
  key('g')
  assert.deepEqual(router.state().menu, ['SPC', 'g'], 'g is a layer at the root, not gg')
  assert.equal(key('j').action.id, 'move.down', 'in SPC g, j closes and moves')
  key(' '); key('c')
  key('h')
  assert.deepEqual(router.state().menu, ['SPC', 'c'], 'SPC c h is a column toggle, not collapse')
  key('Escape')
  key(' '); key('v')
  assert.deepEqual(key('g'), { handled: true, preventDefault: true })
  assert.deepEqual(router.state(), { pending: ['g'], menu: null }, 'g of gg closes the menu and waits')
  assert.equal(key('g').action.id, 'move.top')
  key(' ')
  key('Enter')
  assert.deepEqual(router.state().menu, ['SPC'], 'Enter is not a motion: ignored')
  key(' ')
  // SPC w j exists but is unavailable with the detail on the left: it does not leak into a motion.
  const left = setup({ ctx: browse({ detailAt: 'left' }) })
  left.key(' '); left.key('w')
  assert.equal(left.key('j').action, undefined)
  assert.deepEqual(left.router.state().menu, ['SPC', 'w'])
  assert.equal(left.key('l').action.id, 'focus.right')
  assert.deepEqual(ids(), ['move.down', 'move.bottom', 'move.down', 'cell.toggle', 'move.top'])
})

test('menu: unavailable layers and items cannot be entered; press() acts like the key', () => {
  const noDetail = setup({ ctx: browse({ detail: false }) })
  noDetail.key(' ')
  noDetail.key('w')
  assert.deepEqual(noDetail.router.state().menu, ['SPC'], 'SPC w is not offered without the detail')
  noDetail.key('o')
  assert.deepEqual(noDetail.router.state().menu, ['SPC'], 'SPC o has nothing without the detail')
  const { router, sent } = setup()
  router.press('SPC'); router.press('o')
  assert.equal(router.press('d').action.id, 'option.detailAt')
  assert.deepEqual(router.state().menu, ['SPC', 'o'])
  router.press('Esc')
  assert.equal(router.state().menu, null)
  assert.equal(sent.length, 1)
  // Mode change closes a held menu.
  const r = setup()
  r.key(' '); r.key('v')
  r.state.ctx = { mode: 'sorter' }
  assert.equal(r.key(' ').action.id, 'sorter.toggle')
  assert.equal(r.router.state().menu, null)
})

test('menu: auto-repeats of SPC or of a key inside the menu are ignored; repeats outside it still run', () => {
  const { key, router, ids } = setup()
  key(' ')
  assert.deepEqual(key(' ', { repeat: true }), { handled: true, preventDefault: true })
  assert.deepEqual(router.state().menu, ['SPC'], 'holding SPC keeps the menu open')
  key('v'); key('l')
  key('l', { repeat: true }); key('l', { repeat: true })
  assert.deepEqual(ids(), ['view.deferred'], 'holding a toggle flips it once')
  key('Escape')
  key('j'); key('j', { repeat: true })
  assert.deepEqual(ids(), ['view.deferred', 'move.down', 'move.down'])
})

test('router: a browse → wiki switch drops a held browse menu layer', () => {
  const { key, router, state } = setup()
  key(' '); key('s')
  assert.deepEqual(router.state().menu, ['SPC', 's'])
  state.ctx = { mode: 'wiki', tab: 'wiki', focus: 'list' }
  assert.equal(key('j').action.id, 'move.down', 'the dead sort layer is gone: j moves')
  assert.equal(router.state().menu, null)
})

test('router: wiki mode has its own keys and a wiki-sized SPC menu', () => {
  const { key, ids, router } = setup({ ctx: { mode: 'wiki', tab: 'wiki', focus: 'list' } })
  key('j'); key('h'); key('Tab'); key('Tab', { shiftKey: true }); key('Escape'); key('/'); key('Enter')
  key(' '); key('v'); key('r')
  assert.deepEqual(router.state().menu, ['SPC', 'v'])
  key('Escape'); key(' '); key('s')
  assert.deepEqual(router.state().menu, ['SPC'], 'no sort layer in the wiki')
  key('w'); key('l')
  assert.deepEqual(ids(), ['move.down', 'wiki.back', 'wiki.nextLink', 'wiki.prevLink', 'wiki.clear', 'wiki.search', 'wiki.enter', 'view.raw', 'focus.right'])
})

// ------------------------------------------------------------------ menuLayer, helpSections

test('menuLayer: root items and layers with titles; state labels; availability hides items', () => {
  const ctx = browse({
    tab: 'board',
    hidden: { deferred: true, backlog: false, columns: [false, true] },
    columns: ['todo', 'doing', 'done'],
    sort: [{ field: 'priority', desc: true }],
    cells: { tags: false },
    detailAt: 'left',
    raw: true
  })
  const root = menuLayer(['SPC'], ctx)
  assert.equal(root.title, '메뉴')
  assert.deepEqual(root.items.map((i) => i.key), ['/', 'f', 'g', 'v', 's', 'c', 'o', 'w', '?'])
  const g = root.items.find((i) => i.key === 'g')
  assert.equal(g.kind, 'layer')
  assert.equal(g.label, '+이동')
  assert.deepEqual(g.state, { id: 'board', label: '[칸반]' })

  const view = menuLayer('SPC v', ctx)
  assert.equal(view.title, '보기')
  assert.equal(view.waits, true)
  const byKey = Object.fromEntries(view.items.map((i) => [i.key, i]))
  assert.deepEqual(view.items.map((i) => i.key), ['l', 'b', 'o', 'a', '1', '2', '3', 'd', 'r'], 'only three numbered columns')
  assert.equal(byKey.l.state.label, '[숨김]')
  assert.equal(byKey.b.state.label, '[보임]')
  assert.equal(byKey.l.kind, 'toggle')
  assert.equal(byKey.a.kind, 'action')
  assert.equal(byKey['2'].label, 'doing')
  assert.equal(byKey['2'].state.id, 'hidden')
  assert.equal(byKey.r.state.id, 'raw')

  const sort = Object.fromEntries(menuLayer('SPC s', ctx).items.map((i) => [i.key, i]))
  assert.equal(sort.p.state.label, '[↓ 내림]')
  assert.equal(sort.c.state, null)
  assert.equal(sort.P.state.id, 'desc')
  assert.equal(menuLayer('SPC c', ctx).items.find((i) => i.key === 't').state.id, 'hidden')
  assert.equal(menuLayer('SPC o', ctx).items[0].state.label, '[왼쪽]')
  assert.equal(menuLayer('SPC o', browse()).items[0].state.label, '[아래]')

  // Focus moves follow the split axis; the detail position is gone when the panel is narrow.
  assert.deepEqual(menuLayer('SPC w', ctx).items.map((i) => i.key), ['w', 'W', 'h', 'l'])
  assert.deepEqual(menuLayer('SPC w', browse()).items.map((i) => i.key), ['w', 'W', 'j', 'k'])
  assert.equal(menuLayer('SPC', browse({ detailAtLocked: true })).items.some((i) => i.key === 'o'), false)
  const hiddenDetail = menuLayer('SPC', browse({ detail: false })).items.map((i) => i.key)
  assert.ok(!hiddenDetail.includes('w') && !hiddenDetail.includes('o'))
  assert.ok(!menuLayer('SPC v', browse({ detail: false })).items.some((i) => i.key === 'r'))
  // The wiki menu: tabs, raw, windows, help.
  assert.deepEqual(menuLayer('SPC', { mode: 'wiki' }).items.map((i) => i.key), ['g', 'v', 'w', '?'])
  assert.deepEqual(menuLayer('SPC g', { mode: 'wiki' }).items.map((i) => [i.key, i.action]), [['l', 'tab.list'], ['b', 'tab.board'], ['w', 'tab.wiki']])
})

test('helpSections: grouped rows, platform keys, availability', () => {
  const other = helpSections('browse', browse(), 'other')
  const titles = other.map((s) => s.title)
  assert.deepEqual(titles.slice(0, 2), ['이동', '드나들기·펼침'])
  assert.ok(titles.includes('SPC 메뉴 안에서'))
  const row = (sections, action) => sections.flatMap((s) => s.rows).find((r) => r.action === action)
  assert.deepEqual(row(other, 'move.top').keys, ['g g', 'Home'])
  assert.deepEqual(row(other, 'focus.next').keys, ['SPC w w'])
  assert.deepEqual(row(helpSections('browse', browse(), 'mac'), 'focus.next').keys, ['SPC w w'])
  assert.equal(row(other, 'focus.left').available, false, 'detail below: h/l do nothing')
  assert.equal(row(other, 'focus.down').available, true)
  assert.deepEqual(row(other, 'view.column').keys, ['SPC v 1~9'])
  assert.equal(other.flatMap((s) => s.rows).filter((r) => r.action === 'view.column').length, 1)
  assert.equal(row(other, 'search.open').keys.join(), '/', 'SPC / is folded into the / row')
  const prompt = helpSections('prompt', { mode: 'prompt' }, 'other')
  assert.deepEqual(prompt.map((s) => s.id), ['prompt'])
  assert.equal(row(prompt, 'prompt.deleteWord').keys.join(), 'A-Backspace')
  assert.deepEqual(helpSections('sorter', { mode: 'sorter' }, 'mac').map((s) => s.id), ['sorter'])
  const wiki = helpSections('wiki', { mode: 'wiki' }, 'other')
  assert.equal(row(wiki, 'sort.set'), undefined, 'no sort rows in the wiki help')
  assert.ok(row(wiki, 'wiki.back'))
  assert.equal(MENU_CONTROLS.length, 2)
  // The SPC v 1~9 row stands for all nine columns, not for the first column's name.
  assert.equal(row(helpSections('browse', browse({ columns: ['todo', 'doing'] }), 'other'), 'view.column').label, '1~9번째 칸')
  // The wiki has no up/down panes: its help does not list SPC w j/k.
  assert.equal(row(wiki, 'focus.up'), undefined)
  assert.equal(row(wiki, 'focus.down'), undefined)
  assert.ok(row(wiki, 'focus.left'))
})

test('keyLabel prints keys for kbd hints', () => {
  assert.equal(keyLabel('C-d'), 'Ctrl-d')
  assert.equal(keyLabel('S-Tab'), 'Shift-Tab')
  assert.equal(keyLabel('A-Backspace'), 'Alt-Bksp')
  assert.equal(keyLabel('g g'), 'gg')
  assert.equal(keyLabel('Up'), '↑')
  assert.equal(keyLabel('C-w W'), 'Ctrl-w W')
  assert.equal(keyLabel('SPC w w'), 'SPC w w')
  assert.equal(keyLabel('-'), '-')
})

// ------------------------------------------------------------------ DOM adapter

test('attachKeyRouter: wires keydown, computes the editable flag, calls preventDefault, respects handled events', () => {
  const dom = new JSDOM('<!doctype html><body tabindex="-1"><input id="q"><div id="tabs"></div></body>')
  const doc = dom.window.document
  const sent = []
  const router = createKeyRouter({ platform: 'other', getContext: () => browse(), dispatch: (a) => sent.push(a.id) })
  const detach = attachKeyRouter(doc, router)
  const press = (node, key, init = {}) => {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
    node.dispatchEvent(event)
    return event
  }
  assert.equal(press(doc.body, 'Tab').defaultPrevented, true)
  assert.equal(press(doc.body, 'j').defaultPrevented, true)
  assert.equal(press(doc.getElementById('q'), 'j').defaultPrevented, false)
  assert.equal(press(doc.getElementById('q'), 'Tab').defaultPrevented, false, 'Tab moves focus out of an input')
  assert.equal(press(doc.body, 'x').defaultPrevented, false)
  // A component that handled the key first keeps it.
  doc.getElementById('tabs').addEventListener('keydown', (e) => e.preventDefault())
  press(doc.getElementById('tabs'), 'ArrowLeft')
  assert.deepEqual(sent, ['expandAll', 'move.down'])
  detach()
  press(doc.body, 'j')
  assert.equal(sent.length, 2)
  assert.equal(isEditableTarget(null), false)
  assert.equal(detectPlatform({ platform: 'MacIntel' }), 'mac')
  assert.equal(detectPlatform({ platform: 'Linux x86_64', userAgent: 'X11' }), 'other')
})

// ------------------------------------------------------------------ SPC menu view and help overlay

const menuModule = await import(new URL('view/menu.ts', panel))
const helpModule = await import(new URL('view/help.ts', panel))
const { createMenu, breadcrumb } = menuModule
const { createHelp, kbdHint, focusHint } = helpModule

function menuSetup (ctx = browse()) {
  const { document } = new JSDOM('<!doctype html><body></body>').window
  const state = { ctx }
  const sent = []
  const view = createMenu(document, { press: (k) => router.press(k) })
  const router = createKeyRouter({
    platform: 'other',
    getContext: () => state.ctx,
    dispatch: (a) => { sent.push(a.id); view.update(router.state().menu, state.ctx) },
    onChange: (s) => view.update(s.menu, state.ctx)
  })
  document.body.append(view.el)
  const items = () => [...view.el.querySelectorAll('.m-wk-item')]
  const item = (k) => items().find((n) => n.dataset.key === k)
  return { document, view, router, state, sent, items, item }
}

test('menu: hidden while closed; SPC shows the root grid with kbd keys, labels and "+" layers', () => {
  const m = menuSetup()
  assert.equal(m.view.el.hidden, true)
  assert.equal(m.view.el.children.length, 0)
  m.router.press('SPC')
  assert.equal(m.view.el.hidden, false)
  assert.equal(m.view.el.getAttribute('role'), 'menu')
  const layer = menuLayer(['SPC'], browse())
  assert.deepEqual(m.items().map((n) => n.dataset.key), layer.items.map((i) => i.key))
  const v = m.item('v')
  assert.equal(v.querySelector('kbd').textContent, 'v')
  assert.equal(v.querySelector('.m-wk-label').textContent, '+보기')
  assert.equal(m.view.el.querySelector('.m-wk-title').textContent, '메뉴')
})

test('menu: a layer shows a breadcrumb, state labels and the footer controls; hidden items are absent', () => {
  const m = menuSetup()
  m.router.press('SPC'); m.router.press('v')
  assert.equal(m.view.el.querySelector('.m-wk-title').textContent, 'SPC › 보기')
  assert.equal(breadcrumb(menuLayer(['SPC', 'v'], browse())), 'SPC › 보기')
  const l = m.item('l')
  assert.equal(l.querySelector('.m-wk-label').textContent, '미룸')
  assert.equal(l.querySelector('.m-wk-state').textContent, menuLayer(['SPC', 'v'], browse()).items.find((i) => i.key === 'l').state.label)
  assert.equal(l.getAttribute('role'), 'menuitemcheckbox')
  assert.ok(l.hasAttribute('aria-checked'))
  const footer = m.view.el.querySelector('.m-wk-footer').textContent
  for (const c of MENU_CONTROLS) assert.ok(footer.includes(TABLE.ko.labels[c.label]), c.label)
  assert.ok(footer.includes('Esc') && footer.includes('Bksp'))
  // Wiki mode has no browse-only view toggles: the item is not drawn.
  const w = menuSetup(browse({ mode: 'wiki', tab: 'wiki' }))
  w.router.press('SPC'); w.router.press('v')
  assert.equal(w.item('l'), undefined)
})

test('menu: clicking a toggle is pressing its key; the menu stays open with the new label', () => {
  const m = menuSetup()
  m.router.press('SPC'); m.router.press('v')
  const before = m.item('l').querySelector('.m-wk-state').textContent
  m.state.ctx = browse({ hidden: { deferred: true } })
  m.item('l').click()
  assert.deepEqual(m.sent, ['view.deferred'])
  assert.equal(m.view.el.hidden, false)
  assert.deepEqual(m.router.state().menu, ['SPC', 'v'])
  const after = m.item('l').querySelector('.m-wk-state').textContent
  assert.notEqual(after, before)
  assert.equal(after, menuLayer(['SPC', 'v'], m.state.ctx).items.find((i) => i.key === 'l').state.label)
})

test('menu: clicking an action closes the menu; clicking a layer opens it; Backspace goes up', () => {
  const m = menuSetup()
  m.router.press('SPC')
  m.item('v').click()
  assert.equal(m.view.el.querySelector('.m-wk-title').textContent, 'SPC › 보기')
  m.router.press('Backspace')
  assert.equal(m.view.el.querySelector('.m-wk-title').textContent, '메뉴')
  m.item('g').click()
  m.item('b').click()
  assert.deepEqual(m.sent, ['tab.board'])
  assert.equal(m.view.el.hidden, true)
  m.router.press('SPC')
  m.router.press('Esc')
  assert.equal(m.view.el.hidden, true)
})

test('help: sections, rows with kbd chips, unavailable rows dimmed', () => {
  const { document } = new JSDOM('<!doctype html><body></body>').window
  const help = createHelp(document, { platform: 'mac' })
  document.body.append(help.el)
  assert.equal(help.el.hidden, true)
  const ctx = browse({ detail: false })
  help.open('browse', ctx)
  assert.equal(help.isOpen(), true)
  assert.equal(help.el.getAttribute('role'), 'dialog')
  const sections = helpSections('browse', ctx, 'mac')
  assert.deepEqual([...help.el.querySelectorAll('.m-help-title')].map((n) => n.textContent), sections.map((s) => s.title))
  const rows = [...help.el.querySelectorAll('.m-help-row')]
  assert.equal(rows.length, sections.reduce((n, s) => n + s.rows.length, 0))
  const first = sections[0].rows[0]
  assert.equal(rows[0].querySelector('.m-help-label').textContent, first.label)
  assert.deepEqual([...rows[0].querySelectorAll('kbd')].map((k) => k.textContent), first.keys.map(keyLabel))
  const unavailable = sections.flatMap((s) => s.rows).filter((r) => !r.available)
  assert.ok(unavailable.length > 0, 'some row is unavailable with the detail pane closed')
  assert.equal(help.el.querySelectorAll('[data-available="false"]').length, unavailable.length)
  // Ctrl chords print as Ctrl-.
  assert.ok([...help.el.querySelectorAll('kbd')].some((k) => k.textContent.startsWith('Ctrl-')))
})

test('help: Esc and the close button close it and call onClose', () => {
  const { document, KeyboardEvent } = new JSDOM('<!doctype html><body></body>').window
  let closed = 0
  const help = createHelp(document, { platform: 'other', onClose: () => closed++ })
  document.body.append(help.el)
  help.open('browse', browse())
  help.el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  assert.equal(help.isOpen(), false)
  assert.equal(closed, 1)
  help.open('browse', browse())
  const x = help.el.querySelector('button[aria-label="닫기"]')
  assert.ok(x.querySelector('svg'))
  x.click()
  assert.equal(help.isOpen(), false)
  assert.equal(closed, 2)
  help.close()
  assert.equal(closed, 2)
})

test('help: keys pressed inside the open overlay do not reach the router; closing gives focus back', () => {
  const dom = new JSDOM('<!doctype html><body><button id="list">list</button></body>')
  const doc = dom.window.document
  const sent = []
  const router = createKeyRouter({ platform: 'other', getContext: () => browse(), dispatch: (a) => sent.push(a.id) })
  const detach = attachKeyRouter(doc, router)
  const help = createHelp(doc, { platform: 'other' })
  doc.body.append(help.el)
  const list = doc.getElementById('list')
  list.focus()
  help.open('browse', browse())
  assert.equal(doc.activeElement, help.el)
  for (const key of ['j', 'Tab', ' ', '?']) {
    help.el.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }))
  }
  assert.deepEqual(sent, [])
  assert.equal(router.state().menu, null)
  help.el.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
  assert.equal(help.isOpen(), false)
  assert.equal(doc.activeElement, list, 'focus returns to where it was')
  list.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'j', bubbles: true, cancelable: true }))
  assert.deepEqual(sent, ['move.down'], 'the router works again once the overlay is closed')
  detach()
})

test('help: while open, a key on a control behind it neither clicks nor types; Tab stays inside (se-h5c7)', () => {
  const dom = new JSDOM('<!doctype html><body><button id="b">b</button><input id="i"></body>')
  const doc = dom.window.document
  const help = createHelp(doc, { platform: 'other' })
  doc.body.append(help.el)
  const button = doc.getElementById('b')
  const input = doc.getElementById('i')
  const reached = []
  button.addEventListener('keydown', (e) => reached.push(`button ${e.key}`))
  input.addEventListener('keydown', (e) => reached.push(`input ${e.key}`))
  const press = (node, key, init = {}) => {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
    node.dispatchEvent(event)
    return event
  }
  help.open('browse', browse())
  button.focus()
  assert.equal(press(button, 'Enter').defaultPrevented, true, 'Enter does not click the button behind')
  assert.equal(press(button, ' ').defaultPrevented, true)
  assert.equal(doc.activeElement, help.el, 'the focus comes back to the overlay')
  input.focus()
  assert.equal(press(input, 'a').defaultPrevented, true, 'a letter is not typed into the input behind')
  assert.deepEqual(reached, [], "the controls' own listeners never see the keys")
  assert.equal(press(input, 'c', { metaKey: true }).defaultPrevented, false, "Cmd combinations are the host's")
  // Tab inside cycles the overlay and its close button.
  help.el.focus()
  assert.equal(press(help.el, 'Tab').defaultPrevented, true)
  assert.equal(doc.activeElement, help.el.querySelector('button'))
  press(doc.activeElement, 'Tab')
  assert.equal(doc.activeElement, help.el)
  // Esc from behind closes; then keys reach the controls again.
  input.focus()
  press(input, 'Escape')
  assert.equal(help.isOpen(), false)
  assert.equal(press(button, 'Enter').defaultPrevented, false)
  assert.deepEqual(reached, ['input c', 'button Enter'])
})

test('attachKeyRouter: Tab on a focused control moves the focus; Space on a tab of the tab strip opens the menu (se-h5c7)', () => {
  const dom = new JSDOM('<!doctype html><body><button id="b">b</button><div role="tablist"><button id="t" role="tab" aria-selected="true">t</button></div></body>')
  const doc = dom.window.document
  const sent = []
  const router = createKeyRouter({ platform: 'other', getContext: () => browse(), dispatch: (a) => sent.push(a.id) })
  attachKeyRouter(doc, router)
  const press = (node, key, init = {}) => {
    const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
    node.dispatchEvent(event)
    return event
  }
  const button = doc.getElementById('b')
  const tab = doc.getElementById('t')
  assert.equal(press(button, 'Tab').defaultPrevented, false, 'Tab on a button is not expandAll')
  assert.equal(press(button, 'Tab', { shiftKey: true }).defaultPrevented, false)
  assert.deepEqual(sent, [])
  assert.equal(press(button, ' ').defaultPrevented, false, 'Space still clicks a plain button')
  assert.equal(router.state().menu, null)
  assert.equal(press(tab, 'Enter').defaultPrevented, false, "Enter on a tab is the tab's")
  const space = press(tab, ' ')
  assert.equal(space.defaultPrevented, true, 'Space on a tab does not click it again')
  assert.deepEqual(router.state().menu, ['SPC'], 'it opens the SPC menu')
})

test('help: the router dispatches help for ?', () => {
  const s = setup()
  s.router.handle({ key: '?' })
  assert.deepEqual(s.ids(), ['help'])
})

test('kbdHint: kbd chips with key labels; focus hint text', () => {
  const { document } = new JSDOM('<!doctype html><body></body>').window
  assert.equal(kbdHint(document, '/').querySelector('kbd').textContent, '/')
  assert.deepEqual([...kbdHint(document, ['C-d', 'g g']).querySelectorAll('kbd')].map((k) => k.textContent), ['Ctrl-d', 'gg'])
  assert.equal(focusHint(document).textContent, '페이지를 한 번 누르면 키가 듣습니다')
})

test('focus hint: anchored to the bottom so it never covers the tab bar at the top', async () => {
  const { readFile } = await import('node:fs/promises')
  const css = await readFile(new URL('style/components.css', panel), 'utf8')
  const rule = css.split('\n').find((l) => l.startsWith('.m-focus-hint {'))
  assert.match(rule, /position: fixed/)
  assert.match(rule, /bottom: \d+px/)
  assert.doesNotMatch(rule, /\btop:/)
})

test('keyToken: a Korean input source falls back to event.code (jamo, Process), ASCII keys stay', () => {
  assert.equal(keyToken({ key: 'ㅓ', code: 'KeyJ' }), 'j')
  assert.equal(keyToken({ key: 'ㅎ', code: 'KeyG', shiftKey: true }), 'G')
  assert.equal(keyToken({ key: 'ㅎ', code: 'KeyG' }), 'g')
  assert.equal(keyToken({ key: 'Process', code: 'KeyK', keyCode: 229 }), 'k')
  assert.equal(keyToken({ key: 'ㅓ', code: 'KeyJ', ctrlKey: true }), 'C-j')
  // AZERTY: 'a' typed on the physical KeyQ is still 'a'.
  assert.equal(keyToken({ key: 'a', code: 'KeyQ' }), 'a')
  // No Key*/Digit* code: what it was before.
  assert.equal(keyToken({ key: 'ㅓ' }), 'ㅓ')
  assert.equal(keyToken({ key: 'ㅓ', code: 'Space' }), 'ㅓ')
  assert.equal(keyToken({ key: 'Process' }), null)
})

test('router: Korean letters run the vim keys in a non-editable target, composing in an editable one is ignored', () => {
  const { key, ids } = setup()
  const r = key('ㅓ', { code: 'KeyJ' })
  assert.equal(r.handled, true)
  const p = key('Process', { code: 'KeyK', keyCode: 229, isComposing: false })
  assert.equal(p.handled, true)
  assert.equal(ids().length >= 2, true)
  const e = key('ㅓ', { code: 'KeyJ', isComposing: true, editable: true })
  assert.equal(e.handled, false)
  assert.equal(key('Process', { code: 'KeyK', keyCode: 229, editable: true }).handled, false)
  assert.equal(key('ㅓ', { isComposing: true }).handled, false)
})

test('help: while open, a paste, cut, undo or redo into an input behind it is stopped; copy and select-all go on', () => {
  for (const [platform, mod] of [['other', 'ctrlKey'], ['mac', 'metaKey']]) {
    const dom = new JSDOM('<!doctype html><body><input id="i"><button id="b">b</button></body>')
    const doc = dom.window.document
    const help = createHelp(doc, { platform })
    doc.body.append(help.el)
    const input = doc.getElementById('i')
    const press = (node, key, init = {}) => {
      const event = new dom.window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
      node.dispatchEvent(event)
      return event
    }
    help.open('browse', browse())
    for (const [key, init] of [['v', {}], ['x', {}], ['z', {}], ['Z', { shiftKey: true }], ['y', {}]]) {
      input.focus()
      assert.equal(press(input, key, { [mod]: true, ...init }).defaultPrevented, true, `${platform} ${mod}+${key} edits the input behind`)
      assert.equal(doc.activeElement, help.el, 'the focus comes back to the overlay')
    }
    for (const key of ['c', 'a']) {
      input.focus()
      assert.equal(press(input, key, { [mod]: true }).defaultPrevented, false, `${platform} ${mod}+${key} edits nothing: the host's`)
      assert.equal(doc.activeElement, input)
    }
    // On a control that is no input, the same combination edits nothing and stays the host's.
    const button = doc.getElementById('b')
    button.focus()
    assert.equal(press(button, 'v', { [mod]: true }).defaultPrevented, false)
    assert.equal(help.isOpen(), true)
  }
})

test('table: SPC v a says the archive stays out, as SHOW_ALL leaves it (se-q0u4)', () => {
  const item = menuLayer(['SPC', 'v'], browse()).items.find((one) => one.key === 'a')
  assert.equal(item.action, 'view.all')
  assert.match(item.label, /아카이브 빼고/)
  const row = helpSections('browse', browse(), 'other').flatMap((s) => s.rows).find((r) => r.action === 'view.all')
  assert.match(row.label, /아카이브 제외/)
  // The list note's button runs the same SHOW_ALL and says the same (9w2 #13).
  for (const lang of ['en', 'ko', 'ja', 'zh']) assert.ok(TABLE[lang].labels['view.allHelp'].includes(copyFor(lang).list.showAll.split(' ')[0]), lang)
})

test('menu: the menu label fits its cell — SPC v a in every language, and the whole label is the title', () => {
  // About 126px are left for a label in a which-key cell at 13px: count a wide character as 13px, a narrow one as 7.
  const width = (text) => [...text].reduce((sum, ch) => sum + (/[\u1100-\uffff]/.test(ch) ? 13 : 7), 0)
  for (const [lang, words] of Object.entries(TABLE)) assert.ok(width(words.labels['view.all']) <= 126, `${lang}: ${words.labels['view.all']}`)
  const m = menuSetup()
  m.router.press('SPC'); m.router.press('v')
  const label = m.item('a').querySelector('.m-wk-label')
  assert.equal(label.title, label.textContent)
})

test('menu: no label in any layer outgrows its cell in any language, and the help keeps the long words (se-kd3u)', () => {
  const width = (text) => [...text].reduce((sum, ch) => sum + (/[\u1100-\uffff]/.test(ch) ? 13 : 7), 0)
  const ctx = browse()
  for (const [lang, words] of Object.entries(TABLE)) {
    for (const path of [['SPC'], ['SPC', 'v'], ['SPC', 's'], ['SPC', 'c'], ['SPC', 'o'], ['SPC', 'g'], ['SPC', 'w']]) {
      for (const item of menuLayer(path, ctx, words).items) assert.ok(width(item.label) <= 126, `${lang} ${path.join(' ')} ${item.key}: ${item.label}`)
    }
  }
  const rows = helpSections('browse', ctx, 'other', TABLE.en).flatMap((s) => s.rows)
  const longer = (action) => rows.find((r) => r.action === action).label
  assert.equal(longer('option.detailAt'), 'Details position (left ↔ bottom)')
  assert.equal(longer('view.raw'), 'Raw or rendered')
  assert.equal(rows.find((r) => r.action === 'sort.add' && r.args?.field === 'priority').label, 'Priority (add at the end)')
})

test('toolbar: the scope select is capped, so its disabled notes line cannot widen it (se-s3vv)', async () => {
  const { readFileSync } = await import('node:fs')
  const css = readFileSync(new URL('style/components.css', panel), 'utf8')
  const rule = css.match(/\.m-search \.m-select \{[^}]*\}/)?.[0]
  assert.ok(rule, 'a rule for the search box select')
  assert.match(rule, /max-width:\s*\d+(\.\d+)?(em|px|ch)/)
})
