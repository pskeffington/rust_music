#!/usr/bin/env python3
"""Read-only, stdlib-only validator for Rust Music's Codex agent/roadmap index.

Developer utility; never required by the Rust player or scripts/offline-gate.sh.
Requires Python 3.11+ for standard-library tomllib.
"""
from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "docs" / "roadmap-index.json"
GATE_STATES = {"source_implemented", "partial_evidence", "planned"}
TASK_STATES = {"planned", "blocked", "needs_operator", "in_progress", "done"}
ROLES = {"read_only", "write_scoped"}
EVIDENCE_CLASSES = {
    "source_only", "operator_reported", "locally_executed",
    "independent_pcm_parity", "hardware_measured",
}
PRIORITIES = {"P0", "P1", "P2"}
REQUIRED_FILES = (
    "AGENTS.md",
    ".codex/config.toml",
    "docs/ROADMAP.md",
    "docs/CODEX_AGENT_MAP.md",
    "docs/REPOSITORY_INDEX.md",
    "docs/literature-index.json",
    "docs/lossless-formats.json",
)


class InvalidIndex(ValueError):
    """Stable, operator-actionable index validation error."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise InvalidIndex(message)


def nonempty(value: object, context: str) -> str:
    require(isinstance(value, str) and bool(value.strip()), context)
    return value


def local_path(root: Path, raw: object, label: str) -> Path:
    name = nonempty(raw, label)
    posix = PurePosixPath(name.rstrip("/"))
    require(not posix.is_absolute() and ".." not in posix.parts, f"{label}: unsafe path {name!r}")
    require(name == name.replace("\\", "/") and not name.startswith("./"), f"{label}: noncanonical path {name!r}")
    path = root.joinpath(*posix.parts)
    require(path.exists(), f"{label}: missing path {name!r}")
    require(path.resolve().is_relative_to(root.resolve()), f"{label}: path escapes repository {name!r}")
    return path


def validate_registry(data: dict, root: Path = ROOT, check_files: bool = True) -> None:
    require(isinstance(data, dict), "top-level index must be a JSON object")
    require(data.get("schema_version") == 1, "unsupported roadmap index schema")
    require(data.get("repository") == "pskeffington/rust_music", "wrong repository")
    require(data.get("active_development_branch") == "build/native-codecs-g0",
            "branch changed: review mapping before updating this field")
    require(data.get("draft_pr") == 1, "unexpected draft PR reference")

    policy = data.get("policy")
    require(isinstance(policy, dict), "policy missing")
    for key, expected in (
        ("runtime_cargo_dependencies", "local-workspace-only"),
        ("runtime_network", False),
        ("decoder_ownership", "locally-authored-only"),
        ("playback_lossless_only", True),
        ("minimum_playback_pcm_bits", 16),
        ("minimum_playback_sample_rate_hz", 44100),
        ("bit_perfect_hardware_verified", False),
    ):
        require(policy.get(key) == expected, f"lossless policy mismatch: {key}")
    require(isinstance(policy.get("unsupported_lossy_codecs"), list)
            and {"AAC", "MP3", "Opus", "Vorbis"} <= set(policy["unsupported_lossy_codecs"]),
            "lossy exclusion list incomplete")
    classes = data.get("evidence_classification")
    require(isinstance(classes, list) and set(classes) == EVIDENCE_CLASSES
            and len(classes) == len(EVIDENCE_CLASSES), "unrecognized evidence classes")

    agents = data.get("agent_definitions")
    require(isinstance(agents, list) and len(agents) >= 4, "missing Codex agent map")
    role_by_id = {}
    writer_scopes = []
    for agent in agents:
        require(isinstance(agent, dict), "agent record must be an object")
        role = nonempty(agent.get("id"), "agent id")
        require(re.fullmatch(r"[a-z][a-z0-9_]*", role) is not None, f"invalid agent ID {role}")
        require(role not in role_by_id, f"duplicate agent ID {role}")
        role_by_id[role] = agent
        require(agent.get("mode") in ROLES, f"{role}: invalid mode")
        require(isinstance(agent.get("owns"), list) and isinstance(agent.get("reads"), list),
                f"{role}: file scopes must be arrays")
        require(nonempty(agent.get("purpose"), f"{role}: purpose"), f"{role}: purpose")
        if agent["mode"] == "read_only":
            require(not agent["owns"], f"{role}: read-only agent cannot own write scopes")
        else:
            require(agent["owns"], f"{role}: writing agent has no owned paths")
            writer_scopes.extend((role, value.rstrip("/")) for value in agent["owns"])
        if check_files:
            cfg = local_path(root, agent.get("config"), f"{role}: TOML agent definition")
            toml = tomllib.loads(cfg.read_text(encoding="utf-8"))
            require(toml.get("name") == role, f"{role}: TOML name does not match registry")
            require(nonempty(toml.get("description"), f"{role}: description"), f"{role}: description")
            require(len(nonempty(toml.get("developer_instructions"), f"{role}: instructions")) >= 100,
                    f"{role}: instructions too short")
            require("model" not in toml, f"{role}: avoid hard-pinning account-specific models")
            for path in agent["owns"] + agent["reads"]:
                local_path(root, path, f"{role}: scope")

    for i, (a_role, a) in enumerate(writer_scopes):
        for b_role, b in writer_scopes[i + 1:]:
            if a_role != b_role:
                require(not (a == b or a.startswith(b + "/") or b.startswith(a + "/")),
                        f"overlapping writing scopes: {a_role}:{a} vs {b_role}:{b}")

    gates = data.get("gates")
    require(isinstance(gates, list) and len(gates) == 15, "expected G0-G14 gates")
    gate_ids = [nonempty(g.get("id"), "gate ID") for g in gates]
    require(gate_ids == [f"G{i}" for i in range(15)], "gates must be unique, ordered G0..G14")
    for gate in gates:
        gid = gate["id"]
        require(gate.get("status") in GATE_STATES, f"{gid}: unknown status")
        require(gate.get("owner_agent") in role_by_id, f"{gid}: unknown owner agent")
        nonempty(gate.get("title"), f"{gid}: title")
        nonempty(gate.get("next_evidence"), f"{gid}: next evidence")
        if check_files:
            local_path(root, gate.get("evidence_doc"), f"{gid}: evidence document")

    tasks = data.get("tasks")
    require(isinstance(tasks, list) and tasks, "missing task queue")
    ids = [nonempty(t.get("id"), "task ID") for t in tasks]
    require(ids == [f"RM-{i:03d}" for i in range(1, len(ids) + 1)],
            "task IDs must be unique, sequential, stable RM-001..")
    id_to_task = dict(zip(ids, tasks))
    for task in tasks:
        tid = task["id"]
        require(task.get("gate") in gate_ids, f"{tid}: unknown gate")
        require(task.get("owner_agent") in role_by_id, f"{tid}: unknown owner")
        require(task.get("review_agent") in role_by_id
                and task["review_agent"] != task["owner_agent"], f"{tid}: invalid reviewer")
        require(task.get("status") in TASK_STATES, f"{tid}: unknown status")
        require(task.get("priority") in PRIORITIES, f"{tid}: invalid priority")
        nonempty(task.get("title"), f"{tid}: title")
        nonempty(task.get("acceptance"), f"{tid}: acceptance")
        nonempty(task.get("blocker"), f"{tid}: blocker or qualification")
        require(isinstance(task.get("depends_on"), list), f"{tid}: dependencies must be an array")
        require(isinstance(task.get("focus_paths"), list) and task["focus_paths"],
                f"{tid}: at least one focus path required")
        require(isinstance(task.get("create_paths"), list), f"{tid}: create_paths must be an array")
        if task["status"] == "blocked":
            require(task["depends_on"], f"{tid}: blocked task requires a dependency")
        if task["status"] == "needs_operator":
            require(not task["depends_on"], f"{tid}: operator gate unexpectedly depends on another task")
        for dep in task["depends_on"]:
            require(dep in id_to_task and dep != tid, f"{tid}: missing/self dependency {dep}")
            require(int(dep[3:]) < int(tid[3:]), f"{tid}: dependency order/cycle for {dep}")
            if task["status"] == "done":
                require(id_to_task[dep]["status"] == "done",
                        f"{tid}: cannot close before {dep} closes")
        if check_files:
            for path in task["focus_paths"]:
                local_path(root, path, f"{tid}: focus path")
            for path in task["create_paths"]:
                parent = PurePosixPath(nonempty(path, f"{tid}: create path")).parent.as_posix()
                local_path(root, parent, f"{tid}: future parent")
                require(not (root / path).exists(), f"{tid}: create path already exists: {path}")

    require(all(isinstance(v, str) and v for v in data.get("shared_write_paths", [])),
            "shared_write_paths must contain nonempty paths")
    if check_files:
        for name in REQUIRED_FILES + (
            nonempty(data.get("canonical_human_roadmap"), "roadmap path"),
            nonempty(data.get("codex_routing"), "Codex map path"),
            nonempty(data.get("navigation"), "navigation path"),
        ):
            local_path(root, name, "required file")
        for path in data.get("related_registries", []):
            local_path(root, path, "related index")
        for path in data["shared_write_paths"]:
            local_path(root, path, "shared path")

        config = tomllib.loads((root / ".codex/config.toml").read_text(encoding="utf-8"))
        settings = config.get("agents", {})
        require(settings.get("enabled") is True, "Codex agents disabled")
        require(isinstance(settings.get("max_concurrent_threads_per_session"), int)
                and 1 <= settings["max_concurrent_threads_per_session"] <= 3,
                "agent concurrency exceeds planned nonoverlap boundary")

        docs = {
            "roadmap": (root / data["canonical_human_roadmap"]).read_text(encoding="utf-8"),
            "agents": (root / data["codex_routing"]).read_text(encoding="utf-8"),
            "navigation": (root / data["navigation"]).read_text(encoding="utf-8"),
            "readme": (root / "README.md").read_text(encoding="utf-8"),
            "root": (root / "AGENTS.md").read_text(encoding="utf-8"),
        }
        for gate in gates:
            require(f"**{gate['id']}**" in docs["roadmap"]
                    and gate["title"] in docs["roadmap"],
                    f"{gate['id']}: roadmap gate table stale")
        for task in tasks:
            require(f"`{task['id']}`" in docs["roadmap"]
                    and task["title"] in docs["roadmap"],
                    f"{task['id']}: roadmap task table stale")
        for agent in agents:
            require(f"`{agent['id']}`" in docs["agents"]
                    and f"`{agent['config']}`" in docs["agents"],
                    f"{agent['id']}: agent table stale")
        for doc_key in ("roadmap", "agents", "navigation", "root"):
            require("roadmap-index.json" in docs[doc_key],
                    f"{doc_key}: missing machine index link")
        for filename in ("CODEX_AGENT_MAP.md", "REPOSITORY_INDEX.md", "roadmap-index.json"):
            require(filename in docs["readme"], f"README missing {filename}")
        formats = json.loads((root / "docs/lossless-formats.json").read_text(encoding="utf-8"))
        floor = formats.get("playback_floor", {})
        require(floor.get("integer_pcm_bits") == policy["minimum_playback_pcm_bits"]
                and floor.get("sample_rate_hz") == policy["minimum_playback_sample_rate_hz"],
                "format registry disagrees with roadmap lossless floor")
        require(formats.get("source_integrity", {}).get("bit_perfect_hardware_verified") is False,
                "unmeasured hardware fidelity cannot be marked bit-perfect")
        format_records = formats.get("formats", [])
        require(isinstance(format_records, list) and format_records,
                "lossless format registry has no codec entries")
        format_ids = [record.get("id") for record in format_records]
        require(len(format_ids) == len(set(format_ids)), "duplicate format IDs")
        for record in format_records:
            require(record.get("gate") in gate_ids or record.get("gate") == "future",
                    f"format {record.get('id')}: gate not indexed in roadmap")
            require(record.get("stage") in {
                "native-source-implemented", "planned", "later", "optional"
            }, f"format {record.get('id')}: invalid stage")
            require(record.get("accepted_codecs"), f"format {record.get('id')}: empty codec whitelist")


def check() -> None:
    require(INDEX.stat().st_size <= 512_000, "roadmap JSON exceeds 512 KiB")
    data = json.loads(INDEX.read_text(encoding="utf-8"))
    validate_registry(data, ROOT)
    statuses = {s: sum(t["status"] == s for t in data["tasks"]) for s in TASK_STATES}
    print(f"OK: {len(data['gates'])} gates, {len(data['tasks'])} tasks, "
          f"{len(data['agent_definitions'])} Codex agents, "
          f"{len(data['shared_write_paths'])} shared-path locks; "
          f"needs_operator={statuses['needs_operator']}, blocked={statuses['blocked']}")


if __name__ == "__main__":
    try:
        check()
    except (InvalidIndex, KeyError, TypeError, ValueError, OSError, tomllib.TOMLDecodeError) as exc:
        print(f"INVALID Rust Music roadmap index: {exc}", file=sys.stderr)
        sys.exit(1)
