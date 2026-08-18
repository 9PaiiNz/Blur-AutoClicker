#!/usr/bin/env node
/*
 * Custom `npm run check` orchestrator.
 *
 * Runs every quality gate in order with live per-step progress, collects the
 * real output of each step, and ends with a clear verdict. On a fully clean run
 * it prints only a success card (no noisy summary); when something warns or
 * fails it prints a result table plus the captured output tail of each problem.
 *
 * `check` is read-only and CI-safe. `check --fix` additionally auto-fixes the
 * failing formatting/lint checks (prettier, eslint, cargo fmt, npm audit) and
 * re-verifies them — it never auto-edits code that only fails type/test checks.
 */

import { spawnSync } from 'node:child_process';

const COLOR =
  (process.stdout.isTTY || process.env.FORCE_COLOR) &&
  !process.env.NO_COLOR &&
  process.env.TERM !== 'dumb';
const esc = (code) => (s) => (COLOR ? `\x1b[${code}m${s}\x1b[0m` : `${s}`);
const C = {
  bold: esc('1'),
  dim: esc('2'),
  red: esc('31'),
  green: esc('32'),
  yellow: esc('33'),
  cyan: esc('36'),
  grey: esc('90'),
  reset: COLOR ? '\x1b[0m' : '',
};
const S = COLOR
  ? { pass: '✔', warn: '⚠', fail: '✖', run: '▶', rerun: '↻' }
  : { pass: 'ok', warn: '!!', fail: 'XX', run: '>', rerun: '>' };

/** @typedef {{ name: string, cmd: string[], warn?: RegExp[], fix?: string[] }} Check */

/** @type {Check[]} */
const CHECKS = [
  {
    name: 'cargo test',
    cmd: ['cargo', 'test', '--manifest-path', 'src-tauri/Cargo.toml'],
    warn: [/^warning(\[\w+\])?:/m],
  },
  { name: 'npm test', cmd: ['npm', 'run', 'test'], warn: [/^warning:/m] },
  {
    name: 'eslint',
    cmd: ['npm', 'run', 'lint'],
    warn: [/\bwarning\b/i],
    fix: ['npm', 'run', 'lint', '--', '--fix'],
  },
  { name: 'prettier', cmd: ['npm', 'run', 'format:check'], fix: ['npm', 'run', 'format:write'] },
  { name: 'frontend:build', cmd: ['npm', 'run', 'frontend:build'], warn: [/warning/i] },
  {
    name: 'cargo check',
    cmd: ['cargo', 'check', '--manifest-path', 'src-tauri/Cargo.toml', '--locked'],
    warn: [/^warning(\[\w+\])?:/m],
  },
  {
    name: 'clippy',
    cmd: ['cargo', 'clippy', '--manifest-path', 'src-tauri/Cargo.toml'],
    warn: [/^warning(\[\w+\])?:/m],
  },
  {
    name: 'fmt',
    cmd: ['cargo', 'fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--check'],
    fix: ['cargo', 'fmt', '--manifest-path', 'src-tauri/Cargo.toml'],
  },
  { name: 'npm audit', cmd: ['npm', 'audit'], fix: ['npm', 'audit', 'fix'] },
];

/**
 * @param {Check} c
 * @returns {{ name: string, status: 'pass'|'warn'|'fail', out: string, ms: number, code: number|null }}
 */
function run(c) {
  const t0 = Date.now();
  const r = spawnSync(c.cmd[0], c.cmd.slice(1), {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    shell: true,
  });
  const ms = Date.now() - t0;
  const out = r.error ? String(r.error) : (r.stdout || '') + (r.stderr || '');
  let status = 'pass';
  if (r.status !== 0) status = 'fail';
  else if (c.warn && c.warn.some((re) => re.test(out))) status = 'warn';
  return { name: c.name, status, out, ms, code: r.status };
}

/** Bordered single-line card. */
function card(text, color) {
  const w = [...text].length;
  const bar = '─'.repeat(w + 2);
  return color(`┌${bar}┐\n│ ${text} │\n└${bar}┘`);
}

const total = CHECKS.length;
const doFix = process.argv.includes('--fix');
const results = [];
const t0 = Date.now();

process.stdout.write(`${C.bold('Quality checks')} ${C.grey(`(${total})`)}\n`);
for (let i = 0; i < total; i++) {
  const c = CHECKS[i];
  process.stdout.write(`${C.cyan(S.run)} [${i + 1}/${total}] ${C.bold(c.name)} … `);
  const res = run(c);
  const tag =
    res.status === 'pass'
      ? `${C.green(S.pass)} passed`
      : res.status === 'warn'
        ? `${C.yellow(S.warn)} warnings`
        : `${C.red(S.fail)} failed`;
  process.stdout.write(`${tag} ${C.grey(`(${res.ms}ms)`)}\n`);
  results.push(res);
}

if (doFix) {
  for (let i = 0; i < total; i++) {
    const c = CHECKS[i];
    if (results[i].status !== 'fail' || !c.fix) continue;
    process.stdout.write(`${C.dim(`${S.rerun} auto-fixing ${c.name}…`)}${C.reset}\n`);
    const fr = spawnSync(c.fix[0], c.fix.slice(1), {
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
      shell: true,
    });
    if (fr.status !== 0) process.stdout.write(`${C.dim(`  (fixer exited ${fr.status})`)}${C.reset}\n`);
    const res = run(c);
    const tag =
      res.status === 'pass'
        ? `${C.green(S.pass)} fixed`
        : res.status === 'warn'
          ? `${C.yellow(S.warn)} still warnings`
          : `${C.red(S.fail)} still failing`;
    process.stdout.write(`  ${tag} ${C.grey(`(${res.ms}ms)`)}\n`);
    results[i] = res;
  }
}

const fails = results.filter((r) => r.status === 'fail');
const warns = results.filter((r) => r.status === 'warn');
const totalMs = Date.now() - t0;

if (fails.length === 0 && warns.length === 0) {
  process.stdout.write(`\n${card(`✔  All ${total} quality checks passed  (${totalMs}ms)`, C.green)}\n`);
  process.exit(0);
}

process.stdout.write(`\n${C.bold('Summary')}\n`);
const nameW = Math.max(...results.map((r) => r.name.length));
for (const r of results) {
  const mark =
    r.status === 'pass' ? C.green(S.pass) : r.status === 'warn' ? C.yellow(S.warn) : C.red(S.fail);
  process.stdout.write(`  ${mark} ${r.name.padEnd(nameW)} ${C.grey(`${r.ms}ms`)}\n`);
}

for (const r of [...warns, ...fails]) {
  process.stdout.write(
    `\n${r.status === 'fail' ? C.red : C.yellow}${C.bold(`${r.name} — ${r.status}`)}${C.reset}\n`,
  );
  const lines = r.out.replace(/\r\n/g, '\n').trim().split('\n');
  process.stdout.write(`${C.dim}${lines.slice(-60).join('\n')}${C.reset}\n`);
}

const verdict =
  fails.length > 0
    ? `✖  ${fails.length} check(s) failed`
    : `⚠  ${warns.length} check(s) passed with warnings`;
process.stdout.write(`\n${card(verdict, fails.length > 0 ? C.red : C.yellow)}\n`);
process.exit(fails.length > 0 ? 1 : 0);
