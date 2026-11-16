import unittest
from pathlib import Path
import numpy as np

from speech_benchmark import Matcher, sample_windows, removals, private_output
from compare_speech import score, event_counts, edit_counts, alignment_score, validate_events, model_files
from benchmark_uhm import CORE_FRAMES, HOP, probabilities, intervals, screen
from prepare_retake_review import checked_anchors, disjoint_pair
from prepare_gap_review import best_counterpart, ranked_gaps
from audit_cut_gap import account


class RetakeReviewTests(unittest.TestCase):
    def test_gap_accounting_partitions_retained_audio_and_preserves_reordered_matches(self):
        anchors = [{"source": (0, 2), "reference": (0, 2)},
                   {"source": (20, 22), "reference": (2, 4)},
                   {"source": (8, 10), "reference": (4, 6)}]
        result = account([(0, 25)], anchors)
        self.assertEqual(result["matched_source_seconds"], 6)
        self.assertEqual(result["between_adjacent_reference_anchors_seconds"], 16)
        self.assertEqual(result["unresolved_source_seconds"], 3)

    def test_skipped_gap_excludes_reordered_export_audio_and_confirmed_takes(self):
        anchors = [{"source": (0, 2), "reference": (0, 2)},
                   {"source": (20, 22), "reference": (2, 4)},
                   {"source": (8, 10), "reference": (4, 6)}]
        gaps = ranked_gaps(anchors, [(3, 17)], [(6, 7), (15, 18)])
        self.assertEqual([g["retained_seconds"] for g in gaps], [7, 5])
        self.assertEqual([g["eligible"] for g in gaps], [[(10, 15)], [(3, 6), (7, 8)]])

    def test_comparison_requires_ordered_words_and_distinct_source(self):
        item = {"tokens": "head over to instagram and follow the account".split(), "range": (2, 8)}
        match = {**item, "range": (18, 24), "text": "same phrase", "basis": "reference"}
        scrambled = {**match, "tokens": list(reversed(item["tokens"]))}
        self.assertIsNone(best_counterpart(item, [scrambled, {**match, "range": (3, 9)}]))
        self.assertEqual(best_counterpart(item, [match])["ordered_coverage"], 1)

    def test_export_anchor_is_checked_against_actual_waveforms(self):
        original = np.random.default_rng(41).normal(0, 0.08, 32000).astype(np.float32)
        reference = original[4000:20000] * 0.8
        anchor = {"status": "auto_match", "score": 0.96, "reference_start": 0, "reference_end": 2}
        checked = checked_anchors(
            [dict(anchor, source_start=0), dict(anchor, source_start=3999 / 8000)], original, reference
        )
        self.assertEqual(len(checked), 1)
        self.assertGreater(checked[0]["waveform_score"], 0.99)

    def test_no_performance_is_reused_on_either_side_of_a_case(self):
        chosen = {"A": {"range": [1, 4]}, "B": {"range": [9, 12]}}
        self.assertFalse(disjoint_pair({"A": {"range": [11, 14]}, "B": {"range": [17, 20]}}, [chosen]))
        self.assertTrue(disjoint_pair({"A": {"range": [4, 8]}, "B": {"range": [12, 15]}}, [chosen]))


class AcousticWindowTests(unittest.TestCase):
    def test_window_join_preserves_every_frame_through_partial_tail(self):
        expected = np.linspace(0, 1, CORE_FRAMES + 3, dtype=np.float32)
        pcm = np.repeat(expected, HOP)[: -HOP // 2]

        def infer(window):
            scores = np.zeros((1, 1499, 6), dtype=np.float32)
            scores[0, :, 0] = window[0, : 1499 * HOP : HOP]
            scores[0, :, 1] = 1 - scores[0, :, 0]
            return scores

        np.testing.assert_array_equal(probabilities(pcm, infer)[:, 0], expected)

    def test_short_tail_and_saved_speech_are_excluded_from_screen(self):
        scores = np.r_[np.zeros(4), np.ones(6)]
        self.assertEqual(intervals(scores, 0.75, 0.19), [])
        events = intervals(scores, 0.75, 0.2)
        self.assertEqual(len(events), 1)
        self.assertTrue(screen(events, [(0, 0.08)], [], [])[0]["passes_overlap_screen"])
        self.assertFalse(screen(events, [(0.19, 0.23)], [], [])[0]["passes_overlap_screen"])


class AlignmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        rng = np.random.default_rng(831)
        t = np.arange(80000) / 8000
        cls.audio = (rng.normal(0, 0.08, len(t)) * (0.7 + 0.3 * np.sin(t * 7))).astype(np.float32)
        cls.matcher = Matcher(cls.audio, 1.5)

    def test_reordered_exact_performances_match_independently(self):
        for seconds in [7, 1.25]:
            result = self.matcher.match(self.audio[int(seconds * 8000) : int((seconds + 1.5) * 8000)])
            self.assertEqual(result["status"], "auto_match")
            self.assertAlmostEqual(result["source_start"], seconds, places=3)

    def test_identical_repeated_audio_is_ambiguous(self):
        clip = self.audio[8000:24000]
        source = np.concatenate([self.audio[:8000], clip, self.audio[40000:56000], clip, self.audio[64000:]])
        result = Matcher(source, 2).match(clip)
        self.assertEqual(result["status"], "ambiguous")
        self.assertGreater(result["runner_up"], 0.99)

    def test_silence_and_unrelated_audio_are_unmatched(self):
        self.assertEqual(self.matcher.match(np.zeros(12000, dtype=np.float32))["status"], "unmatched")
        other = np.random.default_rng(19).normal(0, 0.1, 12000).astype(np.float32)
        self.assertEqual(self.matcher.match(other)["status"], "unmatched")

    def test_holdout_blocks_have_no_overlapping_excerpts(self):
        samples = sample_windows([], 720, count=24)
        self.assertEqual(len(samples), 24)
        for a in samples:
            self.assertGreaterEqual(a["start"], a["block"] * 120)
            self.assertLessEqual(a["end"], (a["block"] + 1) * 120)
            for b in samples:
                if a["split"] != b["split"]:
                    self.assertTrue(a["end"] <= b["start"] or b["end"] <= a["start"])

    def test_private_output_cannot_land_in_tracked_source_directories(self):
        with self.assertRaises(ValueError):
            private_output(Path(__file__).parent / "private-output-must-fail")

    def test_manual_override_order_and_source_binding(self):
        cut = lambda a, b, d: {
            "range": {"start": int(a * 254016000000), "duration": int((b - a) * 254016000000)},
            "decision": d,
        }
        p = {
            "project": {
                "preedit": {
                    "analyses": {"1": {"source": {"sha256": "a" * 64}}},
                    "maps": {
                        "2": {
                            "analysis": 1,
                            "automatic": [cut(1, 4, "Remove")],
                            "overrides": [cut(2, 3, "Keep"), cut(2.5, 3.5, "Remove")],
                        }
                    },
                }
            }
        }
        self.assertEqual(removals(p, "a" * 64), [[1, 2], [2.5, 4]])
        with self.assertRaises(ValueError):
            removals(p, "b" * 64)


class ScoringTests(unittest.TestCase):
    def test_automatic_alignment_is_not_a_human_verdict(self):
        rows = [{"human_verified": "unreviewed", "reference_start": "0", "reference_end": "2", "source_start": "1"}]
        self.assertIsNone(alignment_score(rows, [[1, 3]])["reference_performance_conflict_seconds"])
        rows[0]["human_verified"] = "yes"
        self.assertEqual(alignment_score(rows, [[2, 3]])["reference_performance_conflict_seconds"], 1)

    def test_event_times_must_stay_in_their_labeled_excerpt(self):
        with self.assertRaises(ValueError):
            validate_events(
                [{"sample_id": "s001", "category": "filler", "start": 0, "end": 1}],
                [{"id": "s001", "start": 100, "end": 120}],
            )

    def test_unreviewed_is_not_zero_error_ground_truth(self):
        label = {
            "id": "s001",
            "verbatim_text": "hello um",
            "transcript_verified": "unreviewed",
            "events_verified": "unreviewed",
        }
        self.assertEqual(score(label, [], []), {"word_errors": None, "fillers": None})

    def test_verified_negative_window_counts_false_fillers(self):
        label = {"id": "s001", "verbatim_text": "", "transcript_verified": "yes", "events_verified": "yes"}
        result = score(label, [{"text": "um", "start": 1, "end": 1.2}], [])
        self.assertEqual(result["fillers"]["fp"], 1)
        self.assertEqual(result["word_errors"]["insertions"], 1)
        self.assertIsNone(result["word_errors"]["wer"])

    def test_missing_filler_counts_as_deletion(self):
        result = edit_counts(["hello", "um", "world"], ["hello", "world"])
        self.assertEqual(result["deletions"], 1)
        self.assertAlmostEqual(result["wer"], 1 / 3)

    def test_predictions_cannot_reuse_a_gold_event(self):
        gold = [{"start": 1, "end": 1.2}]
        result = event_counts(gold, [{"start": 1, "end": 1.2}, {"start": 1.01, "end": 1.21}])
        self.assertEqual((result["tp"], result["fp"], result["fn"]), (1, 1, 0))

    def test_matched_event_timing_error_is_separate_from_detection(self):
        result = event_counts([{"start": 1, "end": 1.2}], [{"start": 1.05, "end": 1.25}])
        self.assertEqual(result["tp"], 1)
        self.assertAlmostEqual(result["boundary_mae_seconds"], 0.05)

    def test_catalogue_rejects_paths_and_unpinned_or_remote_urls(self):
        names = ["config.json", "model.bin", "tokenizer.json", "vocabulary.txt"]
        files = [
            {
                "name": n,
                "url": "https://huggingface.co/Systran/faster-whisper-tiny.en/resolve/" + "a" * 40 + "/" + n,
                "size": 3,
                "sha256": "b" * 64,
            }
            for n in names
        ]
        self.assertEqual(
            len(model_files([{"id": "faster-whisper-tiny.en", "files": files}], "faster-whisper-tiny.en")), 4
        )
        files[0]["name"] = "../escape"
        with self.assertRaises(ValueError):
            model_files([{"id": "faster-whisper-tiny.en", "files": files}], "faster-whisper-tiny.en")
        files[0]["name"] = "config.json"
        files[0]["url"] = "https://example.invalid/config.json"
        with self.assertRaises(ValueError):
            model_files([{"id": "faster-whisper-tiny.en", "files": files}], "faster-whisper-tiny.en")


if __name__ == "__main__":
    unittest.main()
