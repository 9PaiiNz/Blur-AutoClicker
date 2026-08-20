use ```npm run check``` to make sure your code passes all error checking, tests, linting, and formatting checks.
> checks run by ```npm run check```:
     - cargo test
     - npm run lint
     - npm run format:check
     - npm run frontend:build
     - cargo check
     - cargo clippy
     - cargo fmt
     - npm audit

use ```npm run check:fix``` to automatically fix any issues that can be fixed (formatting, eslint, cargo fmt, npm audit). It re-runs the checks afterwards to confirm.
> by default, run ```npm run check:fix``` (not just ```npm run check```) before finishing changes, so auto-fixable issues are resolved automatically and the gate stays green.
