import { test } from 'node:test'
import assert from 'node:assert/strict'
import { releaseVersion as parse } from './release-version.mjs'

test('beta revisions upgrade in order and stable promotes the same version', () => {
  assert.ok(parse('0.3.2-beta.1').versionCode < parse('0.3.2-beta.2').versionCode)
  assert.ok(parse('0.3.2-beta.998').versionCode < parse('0.3.2').versionCode)
  assert.ok(parse('0.3.2').versionCode < parse('0.3.3-beta.1').versionCode)
  assert.ok(parse('0.3.2').versionCode > 301) // Installed legacy 0.3.1.
  assert.equal(parse('0.3.2-beta.1').version, '0.3.2')
})

test('reject ambiguous, overflowing and injected versions', () => {
  for (const value of ['0.3.2-beta.0', '0.3.2-beta.999', '0.100.0', '0.3.100', '999.0.0', '0.3.2;echo bad', '../0.3.2'])
    assert.throws(() => parse(value), value)
})
