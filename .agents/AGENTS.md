## M-Playlist Project Rules

### Mandatory Audit Tracking
Whenever a flaw is discovered, a tool is validated or invalidated, or an architectural decision is made, you MUST automatically update `AUDIT.md` with a timestamped entry containing:
- **FINDING / DECISION:** What was discovered or decided.
- **IMPACT:** Why it matters to the architecture.
- **RESOLUTION:** What was done about it (or mark as PENDING).

Do NOT wait for the user to ask. This is automatic.

### Active Project Documents
| Document | Purpose | Editable? |
|---|---|---|
| `GOAL_v1.md` | The current frozen architectural law for the M-Playlist Engine. | ❌ Only via versioned release (v2, v3...) with user approval. |
| `AUDIT.md` | Timestamped log of all findings, decisions, and test results. | ✅ Append-only. Never delete entries. |
| `BUILD_PLAN.md` | Phased checklist of what to build. Update as items are completed. | ✅ Update as work progresses. |
| `CHANGELOG.md` | Chronological record of project updates. | ✅ Update when architecture or code changes. |
| `TODO.md` | Active tasks based on the Build Plan. | ✅ Update as work progresses. |

### GOAL Versioning
- The latest `GOAL_vN.md` is the frozen, unbreakable architectural law.
- Do NOT silently edit the current GOAL file. Log findings in `AUDIT.md` first.
- Only release a new GOAL version when the user explicitly approves the accumulated changes.

### No Assumptions
- Do NOT assume user approval. Always ask explicitly before committing decisions to project documents.
- Before making architectural plans, read `GOAL_v1.md` first. Do not proceed without this context.
