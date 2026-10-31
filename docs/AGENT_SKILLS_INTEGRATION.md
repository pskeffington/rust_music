# Pinned agent-skills workflow reference

## Selected revision

Rust Music uses the software-agent hierarchy from
[`pskeffington/agent_skills`](https://github.com/pskeffington/agent_skills/tree/6989d835c62f9e0858db56252f1ac7ff6ce65414), pinned at
`6989d835c62f9e0858db56252f1ac7ff6ce65414` (`docs/codex-priorities-20261031`).
That revision descends from the verified hierarchy checkpoint
`42cf3196122feffa8658239fe7947c033bd39913` and preserves its role catalog,
resource chain, release gates, and portable skills while adding the
Codex-priority kickoff and roadmap updates. The selected revision is the more
complete of the two verified references; the additions are workflow guidance,
not Rust Music implementation or release evidence.

The reference was inspected from a separate temporary checkout. The existing
`../agent_skills` checkout was left untouched because it contains unrelated
uncommitted work. Rust Music does not vendor or automatically execute the
external skill tree. The pinned commit and role names are the explicit source
reference; the repository's own `AGENTS.md`, roadmap, specialist TOML files,
and permission boundaries remain authoritative for this product.

## Workflow and Rust Music ownership

The framework's `catalog/roles.json` and `catalog/resource-chain.json` define
an intake → discovery → plan → design → implementation → integration →
independent verification/security/review/documentation → release-decision
sequence. `docs/RELEASE_GATES.md` requires evidence tied to the exact source
commit and fails closed on missing tools or evidence. Its skills are working
instructions, not sandbox permissions, spawned agents, or release approval.

| Pinned framework role | Rust Music responsibility | Existing specialist mapping |
| --- | --- | --- |
| `software-delivery-orchestrator` | Own task selection, dependency order, evidence and handoffs | Coordinator; use `index_curator` for roadmap/index updates |
| `engineering-planning` | Bound work to an RM task and acceptance criteria | Coordinator + `index_curator` |
| `architecture-design` | Review codec, host, IPC and package boundaries | Coordinator; consult the scope owner before shared-interface changes |
| `code-implementation` | Implement one bounded source scope | `codec_engineer`, `audio_backend`, or `host_integrator` |
| `build-integration` | Resolve workspace, platform and package integration | Coordinator with the relevant source owner |
| `test-verification` | Independently execute regression and PCM parity checks | `gate_verifier` |
| `security-assurance` | Read-only parser, dependency, privacy and fail-closed review | `safety_reviewer` |
| `independent-code-review` | Review correctness and regression risk independently | `safety_reviewer` for adversarial checks; coordinator assigns a distinct reviewer where available |
| `documentation-maintenance` | Reconcile ledgers, roadmap and repository navigation | `index_curator` |
| `release-readiness` | Read-only acceptance and evidence decision | `gate_verifier` prepares evidence; human retains release authority |

The framework also defines `engineering-research`; Rust Music routes source and
literature registry maintenance through `index_curator`. The existing project
agents remain the executable Codex routing definitions. Keep one writer per
overlapping file scope; independent security and release reviews must not be
self-approved by the implementer.

## Loaded reference files

For this integration, the selected revision's `AGENTS.md`, role catalog,
resource-chain catalog, `docs/RESOURCE_CHAIN.md`, `docs/RELEASE_GATES.md`, and
`docs/CODEX_PRIORITY_KICKOFF_2026-10-31.md` were read. The following requested
role skills were read: `software-delivery-orchestrator`,
`engineering-planning`, `architecture-design`, `code-implementation`,
`build-integration`, `test-verification`, `security-assurance`,
`independent-code-review`, `documentation-maintenance`, and
`release-readiness`.

## Scope limits

- No external skill code is copied into the product checkout, and no external
  Cargo dependency, runtime network access, credential, or cross-repository
  permission is introduced.
- The framework does not itself provide a permission-enforcing agent runtime;
  Codex's tool and repository permissions continue to control execution.
- The framework's signed release-dossier evaluator is not present in Rust
  Music. Rust Music continues to use its existing offline gate and operator
  evidence runbooks; this reference does not claim signature verification or
  release approval.
- The separate `agent_skills` repository still has local uncommitted changes;
  they were not included or modified during this review.
