#!/usr/bin/env bash
# Configure GitHub branch protection for main.
# Requires: gh CLI, admin access to the repository.
#
# Usage:
#   ./scripts/setup-branch-protection.sh [owner/repo]
#
# Requires all CI jobs to pass before merging.
# Admin enforcement is disabled to allow hotfix pushes.
#
# WARNING: This script overwrites ALL existing branch protection rules.
# If custom checks or restrictions have been added, they will be lost.
#
# NOTE: The Dashboard check requires the dashboard job to run (gated by
# dashboard/tsconfig.json existing). Remove that check entry until the
# dashboard module has committed source code, otherwise PRs may be blocked
# by a missing check context.

set -euo pipefail

REPO="${1:-}"
if [ -z "$REPO" ]; then
  REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || true)"
fi
if [ -z "$REPO" ]; then
  echo "Usage: $0 owner/repo" >&2
  exit 1
fi

echo "Setting branch protection for ${REPO}:main"

gh api "repos/${REPO}/branches/main/protection" \
  --method PUT \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "checks": [
      { "context": "Rust – clippy + test" },
      { "context": "Python – ruff + pytest" },
      { "context": "Dashboard – lint + type-check + build" },
      { "context": "Docker – build check" }
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false,
    "required_approving_review_count": 1
  },
  "restrictions": null,
  "required_conversation_resolution": true
}
EOF

echo "Done. Branch protection requires all CI checks to pass before merging."
