"""Small offline generator tests; no Cargo, benchmark, or external data."""

from collections import Counter
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "benchmark-synthetic.py"
SPEC = importlib.util.spec_from_file_location("benchmark_synthetic", SCRIPT)
synthetic = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(synthetic)


class SyntheticTests(unittest.TestCase):
    def test_fixed_profiles_and_ignored_output(self):
        self.assertEqual(synthetic.PROFILES, (10_000, 100_000, 250_000))
        self.assertEqual(synthetic.REPETITIONS, 100)
        self.assertEqual(synthetic.OUTPUT, synthetic.ROOT / "target/lexicon-stress")
        self.assertEqual(synthetic.SEED_ENTRIES, 271)

    def test_generation_is_byte_identical(self):
        data = synthetic.lexicon_bytes(1024)
        self.assertEqual(data, synthetic.lexicon_bytes(1024))
        self.assertTrue(data.startswith(synthetic.HEADER.encode()))
        self.assertTrue(data.endswith(b"\n"))
        self.assertEqual(len(data.decode().splitlines()) - 1, 1024)
        self.assertLess(len(data), synthetic.MAX_BYTES)
        self.assertEqual(synthetic.digest(data), "1ba43dfcb9f7e96b6e6b26049dd44ffc83f29e7ab71fbbe8a3f60ae01272d324")

    def test_reading_candidate_and_phrase_boundaries(self):
        records = [line.split("\t") for line in synthetic.lexicon_bytes(1024).decode().splitlines()[1:]]
        distribution = Counter(reading for reading, _, _ in records)
        self.assertEqual(max(distribution.values()), 256)
        self.assertEqual(distribution["ㄧ"], 256)
        self.assertEqual(max(len(reading.split(" ")) for reading in distribution), 16)
        self.assertEqual(len({(reading, text) for reading, text, _ in records}), 1024)
        self.assertTrue(all(int(score) > 0 for _, _, score in records))
        self.assertEqual(len({tuple(reading.split()[:2]) for reading in distribution if reading.startswith("ㄨ ㄩ ")}), 1)

    def test_all_profiles_fit_group_capacity(self):
        # Exercise group arithmetic without generating full benchmark profiles.
        for count in synthetic.PROFILES:
            group = (count - synthetic.SEED_ENTRIES - 1) // 256
            self.assertLess(group, 16**3)
            self.assertEqual(len(synthetic.filler_reading(group)), 5)
            # Every filler record is at most 80 bytes. Bound seeds separately.
            seed_size = len(synthetic.lexicon_bytes(synthetic.SEED_ENTRIES))
            self.assertLess(seed_size + (count - synthetic.SEED_ENTRIES) * 80, synthetic.MAX_BYTES)

    def test_invalid_sizes_and_groups_fail(self):
        for count in (0, 270, 250_001):
            with self.assertRaises(ValueError):
                synthetic.lexicon_bytes(count)
            with self.assertRaises(ValueError):
                synthetic.replay_bytes(count)
        for group in (-1, 16**3):
            with self.assertRaises(ValueError):
                synthetic.filler_reading(group)

    def test_common_prefix_group_encoding(self):
        self.assertEqual(synthetic.filler_reading(0), ("ㄨ", "ㄩ", "ㄧ", "ㄧ", "ㄧ"))
        self.assertEqual(synthetic.filler_reading(16), ("ㄨ", "ㄩ", "ㄧ", "ㄨ", "ㄧ"))
        self.assertEqual(len({synthetic.filler_reading(group) for group in range(1000)}), 1000)

    def test_independent_segmentation_expectations(self):
        self.assertEqual(synthetic.decoded_prefix(1), "詞01")
        self.assertEqual(synthetic.decoded_prefix(16), "詞16")
        self.assertEqual(synthetic.decoded_prefix(17), "詞16詞01")
        self.assertEqual(synthetic.decoded_prefix(63), "詞16詞16詞16詞15")
        self.assertEqual(synthetic.decoded_prefix(64), "詞16詞16詞16詞16")

    def test_replay_covers_limit_selection_edit_cancel_and_single_commit(self):
        data = synthetic.replay_bytes(10_000)
        self.assertEqual(data, synthetic.replay_bytes(10_000))
        records = [json.loads(line) for line in data.splitlines()]
        self.assertLessEqual(len(records) * synthetic.REPETITIONS, 1_000_000)
        self.assertTrue(all("expect" in record for record in records))
        self.assertTrue(any(record["expect"].get("status") == "limit_reached" for record in records))
        selection = next(record for record in records if record["event"]["key"] == "down")
        candidates = selection["expect"]["snapshot"]["candidates"]
        self.assertEqual(len(candidates), 256)
        self.assertEqual(candidates[0], {"text": "詞01", "start": 63, "end": 64})
        self.assertEqual(candidates[-1], {"text": "候選255", "start": 63, "end": 64})
        keys = {record["event"]["key"] for record in records}
        self.assertTrue({"home", "end", "backspace", "delete", "page_down", "2", "4", "escape"} <= keys)
        selection_result = next(record for record in records if record["event"]["key"] == "2")
        self.assertEqual(selection_result["expect"]["snapshot"]["preedit"], "詞16詞16詞16詞15候選010")
        tail_page = next(
            record for record in records
            if record["expect"].get("snapshot", {}).get("page") == 28
        )
        self.assertEqual(tail_page["expect"]["snapshot"]["candidate_index"], 252)
        tail_result = next(record for record in records if record["event"]["key"] == "4")
        self.assertEqual(tail_result["expect"]["snapshot"]["preedit"], "詞16詞16詞16詞15候選255")
        self.assertFalse(tail_result["expect"]["snapshot"]["selecting"])
        cancellation = next(record for record in records if record["event"]["key"] == "escape")
        self.assertEqual(cancellation["expect"]["snapshot"]["preedit"], "")
        commits = [index for index, record in enumerate(records) if record["expect"]["commit"] is not None]
        self.assertEqual(len(commits), 3)
        self.assertEqual(records[commits[0]]["expect"]["commit"], "詞16詞16詞16詞16")
        for index in commits:
            self.assertFalse(records[index + 1]["expect"]["consumed"])
            self.assertIsNone(records[index + 1]["expect"]["commit"])

    def test_generator_functions_do_not_run_cli(self):
        with patch.object(synthetic.subprocess, "run", side_effect=AssertionError("unexpected process")):
            synthetic.lexicon_bytes(1024)
            synthetic.replay_bytes(1024)


if __name__ == "__main__":
    unittest.main()
