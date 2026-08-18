#!/usr/bin/env node
/*
 * Custom `npm run fix` orchestrator.
 *
 * Applies every safe auto-fix (formatting, lint --fix, audit fix), then re-runs
 * the full check suite to report what is still outstanding.
 */

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const RESET = '\x1b[0m';
const BOLD = '\x1b[1m';
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const CYAN = '\x1b[36m';
const GREY = '\x1b[90m';

/** @typedef {{ name: string, cmd: string[] }} Fixer */

/** @type {Fixer[]} */
const FIXERS = [
  { name: 'cargo fmt', cmd: ['cargo', 'fmt', '--manifest-path', 'src-tauri/Cargo.toml'] },
  { name: 'prettier --write', cmd: ['npm', 'run', 'format:write'] },
  { name: 'eslint --fix', cmd: ['npm', 'run', 'lint', '--', '--fix'] },
  { name: 'npm audit fix', cmd: ['npm', 'audit', 'fix'] },
];

const total = FIXERS.length;

process.stdout.write(`${BOLD}Running ${total} auto-fixers…${RESET}\n`);
for (let i = 0; i < total; i++) {
  const f = FIXERS[i];
  process.stdout.write(`${CYAN}▶${RESET} [${i + 1}/${total}] ${BOLD}${f.name}${RESET} … `);
  const t0 = Date.now();
  const r = spawnSync(f.cmd[0], f.cmd.slice(1), {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    shell: true,
  });
  const ms = Date.now() - t0;
  if (r.status === 0) {
    process.stdout.write(`${GREEN}✔ done${RESET} ${GREY}(${ms}ms)${RESET}\n`);
  } else {
    const msg = (r.stderr || r.stdout || '').trim().split('\n').slice(-8).join('\n');
    process.stdout.write(`${YELLOW}⚠ exited ${r.status}${RESET} ${GREY}(${ms}ms)${RESET}\n`);
    if (msg) process.stdout.write(`${GREY}${msg}${RESET}\n`);
  }
}

process.stdout.write(`\n${BOLD}Verifying with checks…${RESET}\n`);
const verify = spawnSync(process.execPath, [fileURLToPath(new URL('./check.mjs', import.meta.url))], {
  stdio: 'inherit',
});
process.exit(verify.status ?? 1);
