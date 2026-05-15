# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅ Active development |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please:

1. **Email**: Open a private [GitHub Security Advisory](../../security/advisories/new)
2. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

| Action | Timeline |
|--------|----------|
| Acknowledgment | Within 48 hours |
| Initial assessment | Within 7 days |
| Fix timeline communicated | Within 14 days |
| Patch released | Depends on severity |

## Security Architecture

Agent World's security model:

- **Sandboxed execution** — Agents run in isolated environments
- **Protocol authentication** — A2A messages signed with ed25519
- **Resource limits** — Token system prevents resource exhaustion
- **Rule engine** — Inviolable world rules enforced at engine level
- **Audit trail** — All transactions logged immutably

### Known Security Considerations

- Agent code execution must be sandboxed (no host filesystem access)
- LLM prompt injection is a known attack vector for agents
- Economic exploits (flash crashes, wash trading) need monitoring
- Agent-to-agent trust is reputation-based, not cryptographic

## Responsible Disclosure

We follow responsible disclosure. Security researchers who report vulnerabilities
in good faith will be acknowledged (unless they prefer to remain anonymous).
