#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

git add .gitignore
git commit -m "chore: update gitignore"

git add contracts/vault/Cargo.toml
git commit -m "chore: add rand dev-dependency for fuzz tests"

git add contracts/vault/src/lib.rs
git commit -m "fix: persist revenue_pool and max_deduct in vault init"

git add contracts/vault/src/test.rs
git commit -m "test: large balance and large deduct"

git add PR_MESSAGE.md
git commit -m "docs: add pull request message for issue 32"

git add commit.sh
git commit -m "chore: add per-file commit script"

echo "All commits done."
