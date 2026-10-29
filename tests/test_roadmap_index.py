#!/usr/bin/env python3
"""Adversarial stdlib-only tests for machine roadmap and agent routing."""
import copy
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from check_roadmap_index import InvalidIndex, validate_registry  # noqa: E402


class RoadmapIndexTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.valid = json.loads((ROOT / "docs/roadmap-index.json").read_text(encoding="utf-8"))

    def mutated(self):
        return copy.deepcopy(self.valid)

    def test_index_matches_repo_and_toml(self):
        validate_registry(self.valid, ROOT)

    def test_unknown_gate_is_rejected(self):
        value = self.mutated()
        value["tasks"][0]["gate"] = "G900"
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_dependency_cycle_is_rejected(self):
        value = self.mutated()
        value["tasks"][0]["status"] = "blocked"
        value["tasks"][0]["depends_on"] = ["RM-002"]
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_read_only_agent_cannot_be_a_writer(self):
        value = self.mutated()
        value["agent_definitions"][0]["owns"] = ["crates/rust-music-flac/"]
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_duplicate_agent_is_rejected(self):
        value = self.mutated()
        value["agent_definitions"].append(copy.deepcopy(value["agent_definitions"][0]))
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_lossless_floor_cannot_be_downgraded(self):
        value = self.mutated()
        value["policy"]["minimum_playback_pcm_bits"] = 8
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_blocked_task_must_have_dependency(self):
        value = self.mutated()
        value["tasks"][1]["depends_on"] = []
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_broken_future_task_reference_is_rejected(self):
        value = self.mutated()
        value["tasks"][8]["depends_on"] = ["RM-900"]
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)


if __name__ == "__main__":
    unittest.main()
