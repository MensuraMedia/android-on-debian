export const meta = {
  name: 'review-change',
  description: 'Gate a change: map changed paths to segments, adversarial + deployment review, verify every finding',
  whenToUse: 'Before every release, merge or upgrade of android-on-debian (or an app built with it). Run: "run the review-change workflow" (optionally with args {range: "v0.2.0..HEAD"}).',
  phases: [
    { title: 'Scope', detail: 'list changed files and group them into review segments' },
    { title: 'Review', detail: 'adversarial-reviewer per segment, deploy-reviewer for the rollout' },
    { title: 'Verify', detail: 'two independent skeptics try to refute each finding' },
  ],
}

// args: { range?: string }   e.g. "v0.2.0..HEAD"; default: merge-base with origin/main..HEAD plus uncommitted changes.
// Requires the agents installed by `adx claude-rules install`: adversarial-reviewer, deploy-reviewer.
const range = (args && args.range) || '$(git merge-base HEAD origin/main)..HEAD plus uncommitted and untracked changes (resolve the merge-base with git first)'

const SEGMENTS = {
  type: 'object',
  properties: {
    segments: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          name: { type: 'string', description: 'installer | emulator | aodm-core | aodm-cli-tui | templates | docs | claude-config | ci | other' },
          files: { type: 'array', items: { type: 'string' } },
          focus: { type: 'string', description: 'one line: what changed and what could break' },
        },
        required: ['name', 'files', 'focus'],
      },
    },
  },
  required: ['segments'],
}

const FINDINGS = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          severity: { type: 'string', enum: ['Critical', 'High', 'Medium', 'Low'] },
          where: { type: 'string' },
          what: { type: 'string' },
          evidence: { type: 'string' },
          fix: { type: 'string' },
          confidence: { type: 'string', enum: ['Confirmed', 'Plausible'] },
        },
        required: ['title', 'severity', 'where', 'what', 'evidence', 'fix', 'confidence'],
      },
    },
    verified_ok: { type: 'array', items: { type: 'string' } },
    not_checked: { type: 'array', items: { type: 'string' } },
  },
  required: ['findings'],
}

const VERDICT = {
  type: 'object',
  properties: {
    refuted: { type: 'boolean' },
    reason: { type: 'string' },
  },
  required: ['refuted', 'reason'],
}

phase('Scope')
const scope = await agent(
  `In the android-on-debian repository (the current working directory), list every file changed in ${range} ` +
  `(git diff --name-only, git ls-files --others --exclude-standard). Group them into review segments: ` +
  `installer (adx, lib/, modules/ except emulator, config/*.env, config/skills.manifest, profiles/), emulator (modules/emulator), ` +
  `aodm-core (tools/aodm root files: Cargo.toml, Cargo.lock, deny.toml, rust-toolchain.toml; tools/aodm/crates/aod-core), ` +
  `aodm-cli-tui (other tools/aodm crates), templates (templates/compose-roborazzi), ` +
  `docs (*.md and docs/ not listed elsewhere), claude-config (templates/claude/, config/claude-global-rules.md, config/skill-profiles/, .claude/), ` +
  `ci (.github/). Every file in exactly one segment. Omit empty segments. Read-only.`,
  { label: 'scope', schema: SEGMENTS, effort: 'low' },
)
const segments = (scope && scope.segments) || []
log(`${segments.length} segment(s): ${segments.map(s => s.name).join(', ') || 'none'}`)
if (!segments.length) return { range, segments: [], confirmed: [], refuted: [], note: 'no changes in range' }

const verify = (f, source) =>
  parallel([0, 1].map(i => () =>
    agent(
      `Skeptic ${i + 1}: try to REFUTE this ${source} finding about android-on-debian (current directory), change range ${range}. ` +
      `Re-check the evidence yourself with read-only commands; never edit or install. ` +
      `Refute if it is wrong, already handled, not reproducible, or not tied to the change. Default to refuted=true if you cannot confirm it.\n\n` +
      JSON.stringify(f, null, 2),
      { phase: 'Verify', label: `verify ${i + 1}: ${f.title.slice(0, 40)}`, schema: VERDICT },
    ),
  )).then(votes => ({ ...f, source, votes: votes.filter(Boolean) }))
    .then(f => ({ ...f, status: f.votes.length === 0 ? 'unverified' : f.votes.some(v => !v.refuted) ? 'confirmed' : 'refuted' }))  // confirmed unless every skeptic refuted it

// Segment reviews and the deployment review run side by side; each finding is verified as soon as its review returns.
const [segmentResults, deployResult] = await parallel([
  () => pipeline(
    segments,
    seg => agent(
      `Review segment "${seg.name}" of the change ${range}. Files: ${seg.files.join(', ')}. Focus: ${seg.focus}. ` +
      `Report only defects in or caused by these changes, with evidence.`,
      { phase: 'Review', label: `review ${seg.name}`, agentType: 'adversarial-reviewer', schema: FINDINGS },
    ),
    (rev, seg) => parallel(((rev && rev.findings) || []).map(f => () => verify({ ...f, segment: seg.name }, 'review'))),
  ),
  () => agent(
    `Deployment review of ${range}: upgrade from the previous tag, idempotency, migration of existing state, rollback, ` +
    `clean install, blast radius, CI parity, release hygiene, rollout security.`,
    { phase: 'Review', label: 'deploy review', agentType: 'deploy-reviewer', schema: FINDINGS },
  ).then(rev => parallel(((rev && rev.findings) || []).map(f => () => verify({ ...f, segment: 'deployment' }, 'deployment')))),
])

const all = [...(segmentResults || []).filter(Boolean).flat(), ...((deployResult || []).filter(Boolean))].filter(Boolean)
const order = { Critical: 0, High: 1, Medium: 2, Low: 3 }
const bySeverity = (a, b) => order[a.severity] - order[b.severity]
const confirmed = all.filter(f => f.status === 'confirmed').sort(bySeverity)
const unverified = all.filter(f => f.status === 'unverified').sort(bySeverity)  // no skeptic answered: treat as open, not as fact
const refuted = all.filter(f => f.status === 'refuted').map(f => ({ title: f.title, segment: f.segment, why: f.votes.map(v => v.reason).join(' | ') }))
log(`${confirmed.length} confirmed, ${unverified.length} unverified, ${refuted.length} refuted`)
return { range, segments: segments.map(s => s.name), confirmed, unverified, refuted }
