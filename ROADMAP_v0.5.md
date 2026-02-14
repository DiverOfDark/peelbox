# Peelbox Roadmap to v0.5

## Phase 2: Testing Infrastructure & Stability 🧪
*Goal: Fix flaky tests and expand validation capabilities.*

- **[P2] Caching Validation:** Add container tests that verify:
    - No network access required on second run (offline build).
    - All layers are correctly cached by BuildKit.

## Phase 3: Language & Build System Expansion 🌍
*Goal: Fill the gaps in supported stacks and fixtures.*

- **[P1] Node/NPM Framework Support:** Import missing framework definitions from [Vercel](https://github.com/vercel/vercel/blob/main/packages/frameworks/src/frameworks.ts).
- **[P2] Add Missing Fixtures:**
    - `pipenv` test fixture.
    - Check `peelbox-stack` for all supported stacks and ensure each has a corresponding fixture.
- **[P2] External Examples Integration:** Analyze and add relevant examples from:
    - [Railway Railpack](https://github.com/railwayapp/railpack/tree/main/examples)
    - [Nixpacks](https://github.com/railwayapp/nixpacks/tree/main/examples)
- **[P3] New Integration Tests:**
    - Deno example.
    - Zig test.
    - Bazel multi-language tests.
    - Turborepo multi-stack examples.

## Phase 4: Release 🚀
- **[P1] Version Bump:** After all the above are completed, upgrade version to **0.5.0**.
