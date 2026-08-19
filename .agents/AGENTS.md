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

use ```npm run fix``` to fix any errors that can be automatically fixed. (will run ```npm run check``` after running the fixes to make sure everything is fixed)
> fixes run by ```npm run fix```:
     - cargo fmt
     - prettier --write
     - eslint --fix
     - npm audit fix