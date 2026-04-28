# Commit Message Rules

Use Conventional Commits.

## Format

`type(scope): short summary`

## Allowed Types

- `feat`
- `fix`
- `refactor`
- `test`
- `docs`
- `chore`
- `perf`

## Suggested Scopes

- `wallet-core`
- `wallet-crypto`
- `wallet-transport`
- `wallet-types`
- `wallet-utils`
- `chain-instance`
- `chain-interact`
- `docs`

## Rules

- Use imperative mood.
- Keep the subject line under 72 characters.
- Prefer a scope that matches the crate or doc area being changed.
- Avoid generic summaries like `update code` or `fix stuff`.

## Examples

- `docs(readme): add workspace overview`
- `fix(chain-interact): harden transfer validation`
- `test(wallet-utils): add address parsing coverage`
