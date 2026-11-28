#!/bin/sh
# Install this repository's git hooks; they are templates under scripts/hooks.
set -e
root="$(git rev-parse --show-toplevel)"
chmod +x "$root/scripts/precommit.sh" "$root/scripts/hooks/pre-commit" "$root/scripts/hooks/pre-push"
for hook in pre-commit pre-push; do
    cp "$root/scripts/hooks/$hook" "$root/.git/hooks/$hook"
    chmod +x "$root/.git/hooks/$hook"
done
echo "installed: pre-commit (fmt + host clippy), pre-push (windows clippy + tests)"
