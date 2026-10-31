#!/usr/bin/env python3
"""Offline deterministic validation. Checks provenance, integrity, and derived docs."""
import json
import re
import sys
from pathlib import Path
from urllib.parse import urlsplit
from build_literature import records, render

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"

def check():
    payload = json.loads((DOCS / "literature-index.json").read_text(encoding="utf-8"))
    assert payload == records(), "registry does not match reviewed source-of-truth"
    assert payload["schema_version"] == 1
    roadmap = json.loads((DOCS / "roadmap-index.json").read_text(encoding="utf-8"))
    valid_gates = {gate["id"] for gate in roadmap["gates"]}
    assert valid_gates == {f"G{i}" for i in range(16)}, "roadmap and literature gate catalogs differ"
    ids = [s["id"] for s in payload["sources"]]
    assert ids == [f"LIT-{i:03d}" for i in range(1, len(ids)+1)]
    for s in payload["sources"]:
        assert s["category"] in payload["categories"]
        assert s["dependency_policy"] == "reference-only"
        assert s["gate"] in valid_gates
        assert s["priority"] in {"high", "medium", "low"}
        assert all(s[field].strip() for field in ("title", "decision", "license_note", "validation"))
        url = urlsplit(s["url"])
        assert url.scheme == "https" and url.netloc and not url.username and not url.password
    for i, rule in enumerate(payload["standards"], 1):
        assert rule["id"] == f"RUST-{i:03d}"
        assert set(rule["evidence_ids"]).issubset(ids) and rule["evidence_ids"]
        assert rule["status"] in {"implemented", "planned", "verified"}
        assert rule["gate"] in valid_gates
    expected_matrix, expected_index = render(payload)
    assert (DOCS / "LITERATURE_MATRIX.md").read_text(encoding="utf-8") == expected_matrix
    assert (DOCS / "LITERATURE_INDEX.md").read_text(encoding="utf-8") == expected_index
    assert len(re.findall(r"\*\*LIT-\d{3}\*\*", expected_index)) == len(ids)
    assert len(re.findall(r"\*\*RUST-\d{3}\*\*", expected_matrix)) == len(payload["standards"])
    print(f"OK: {len(ids)} references; {len(payload['standards'])} standards; exact markdown parity")

if __name__ == "__main__":
    try: check()
    except (AssertionError, KeyError, ValueError, TypeError, OSError) as err:
        print(f"invalid Rust Music literature: {err}", file=sys.stderr)
        sys.exit(1)
