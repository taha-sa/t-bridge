export function releaseVersion(input) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-beta\.?(\d+))?$/.exec(input ?? '')
  if (!match) throw new Error('Expected x.y.z or x.y.z-beta.N')
  const [major, minor, patch] = match.slice(1, 4).map(Number)
  if (minor > 99 || patch > 99) throw new Error('Minor and patch versions must be 0..99')
  const beta = match[4] !== undefined
  const revision = beta ? Number(match[4]) : 999
  if (!Number.isInteger(revision) || revision < 1 || revision > (beta ? 998 : 999)) throw new Error('Beta revision must be 1..998')
  const baseCode = major * 10000 + minor * 100 + patch
  const versionCode = baseCode * 1000 + revision
  if (!Number.isSafeInteger(versionCode) || versionCode > 2100000000) throw new Error('Android versionCode is too large')
  return { version: `${major}.${minor}.${patch}`, beta, revision, baseCode, versionCode }
}
