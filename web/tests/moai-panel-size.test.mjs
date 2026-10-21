import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { promisify } from 'node:util'

const exec = promisify(execFile)
const root = new URL('..', import.meta.url).pathname
const cli = join(root, 'packages/sdk/dist/cli.js')
const moai = join(root, 'packages/moai')

// Desktop reads at most 512 KiB of a panel (build.mjs refuses past it). This test stops the panel at 80% of that,
// so a growth that would reach the limit is noticed while there is still room to act.
const PANEL_BUDGET = 419_430

test('the built panel stays within 80% of the 512 KiB Desktop reads', async () => {
  await exec('node', [join(moai, 'build.mjs')])
  const { size } = await stat(join(moai, 'dist/panel.html'))
  assert.ok(size <= PANEL_BUDGET, `dist/panel.html is ${size} bytes; the budget is ${PANEL_BUDGET}`)
})

test('packing the extension twice gives the same archive', async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'saycode-moai-size-'))
  try {
    const digests = []
    for (const name of ['first', 'second']) {
      const archive = join(temporary, `${name}.saycode-extension`)
      await exec('node', [join(moai, 'build.mjs')])
      await exec('node', [cli, 'pack', moai, '--output', archive])
      digests.push(createHash('sha256').update(await readFile(archive)).digest('hex'))
    }
    assert.equal(digests[0], digests[1])
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
})
