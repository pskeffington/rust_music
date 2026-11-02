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

    def test_index_records_current_ref_snapshot(self):
        self.assertEqual(self.valid["active_development_branch"], "codex/mac07-ui-acceptance")
        self.assertEqual(self.valid["integrated_branch"], "main")
        self.assertEqual(self.valid["verified_main_head"], "cf83cf22dee8ffba0e58e32fe83039c2acde733f")

    def test_branch_change_requires_mapping_review(self):
        value = self.mutated()
        value["active_development_branch"] = "build/native-codecs-g0"
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

    def test_mainline_head_change_requires_integration_refresh(self):
        value = self.mutated()
        value["verified_main_head"] = "e8e5ff8864c8f9dffd8e078cb18bc235ef04ee7b"
        with self.assertRaises(InvalidIndex):
            validate_registry(value, ROOT, check_files=False)

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
        value["tasks"][1]["status"] = "blocked"
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
