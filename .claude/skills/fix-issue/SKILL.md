---
name: fix-issue
description: Fix a GitHub issue end-to-end. Reads the issue, creates a branch, implements the fix, runs tests and lints, commits, gets a code review, fixes review feedback, and opens a PR.
disable-model-invocation: true
argument-hint: "[issue-number]"
---

# Fix GitHub Issue

You are executing the `/fix-issue` workflow. Follow each step sequentially. Do NOT skip steps.

## Step 1: Read the Issue

Run `gh issue view $ARGUMENTS --json title,body,labels,milestone` to fetch the issue details. Parse the title and body to understand what needs to be done. If `$ARGUMENTS` is empty or not a valid issue number, ask the user to provide one.

## Step 2: Assign the Issue

Run `gh issue edit $ARGUMENTS --add-assignee @me` to assign the issue to yourself.

## Step 3: Create a Feature Branch

1. Run `git checkout master && git pull` to ensure you're on the latest master.
2. Derive a short slug from the issue title: lowercase, replace spaces/special chars with hyphens, truncate to ~50 chars.
3. Run `git checkout -b feature/$ARGUMENTS-<slug>` to create the branch.

## Step 4: Implement the Fix

Study the issue description and the relevant code. Use the appropriate specialized agent (rust-developer for Rust code, java-spring-expert for Spring, nextjs-fullstack-expert for web, etc.) to implement the fix. Make the minimal changes needed to resolve the issue.

## Step 5: Run Tests

Run `cargo nextest run --features cuda` to execute all tests. If any tests fail, fix them before proceeding. Re-run until all tests pass.

## Step 6: Run Clippy

Run `cargo clippy --all-targets --features cuda -- -D warnings`. If there are any warnings or errors, fix every single one. Re-run clippy until completely clean — zero warnings, zero errors.

## Step 7: Run Formatter

Run `cargo fmt --all` to format all code.

## Step 8: Commit

Stage all changed files and commit using conventional commit format:

```
feat(<scope>): <short description> (#$ARGUMENTS)
```

Where `<scope>` is the affected crate or module (e.g., `detect`, `pipeline`), and `<short description>` summarizes the change. Use `fix` instead of `feat` if the issue is a bug fix, `refactor` for refactoring, etc.

## Step 9: Code Review

Launch the `code-reviewer` subagent (sonnet model) to review the diff between master and your branch:

- Ask it to review the output of `git diff master...HEAD`
- Wait for the review results

## Step 10: Address Review Feedback

Fix all **Critical** and **Important** issues identified by the code reviewer. For **Suggestions**, use your judgment — apply them if they're quick wins, skip if they'd over-engineer the solution.

After fixing:
1. Re-run `cargo nextest run --features cuda` — all tests must pass
2. Re-run `cargo clippy --all-targets --features cuda -- -D warnings` — must be clean
3. Re-run `cargo fmt --all`

If any changes were made, create a second commit:

```
refactor: address code review feedback for #$ARGUMENTS
```

## Step 11: Push and Open PR

1. Push the branch: `git push -u origin feature/$ARGUMENTS-<slug>`
2. Create the PR using `gh pr create` with:
   - Title matching the first commit message
   - Body containing:
     - `## Summary` section with 1-3 bullet points describing what was changed and why
     - `Closes #$ARGUMENTS` to auto-close the issue
     - `## Test plan` section with a checklist of how the changes were verified
