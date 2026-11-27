"""Unit tests for judging_sheet.py: synthetic rows only, no binary, no contest data, no network.

Run: python3 -I -m unittest discover -s eval/dupbench/pairs-swap -p "test_*.py"
"""
from __future__ import annotations

import csv
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import judging_sheet as js  # noqa: E402


def row(contest, issue, role="dup", fires=False, abstained=False, top="1", judged=False, q="query", c="cand"):
    return {"contest": contest, "role": role, "issue": str(issue), "expected_primary": None, "abstained": abstained,
            "alarm_fires": fires, "top_candidate": top, "top_is_judged_primary": judged, "_q": q, "_c": c}


def synthetic(n_alarms=48, n_quiet=80):
    rows = [row("c1", 1000 + i, fires=True, judged=(i % 9 != 0)) for i in range(n_alarms)]
    rows += [row("c2", 2000 + i) for i in range(n_quiet)]
    return rows


class Trim(unittest.TestCase):
    def test_short_text_unchanged(self):
        self.assertEqual(js.trim("abc"), "abc")

    def test_exactly_cap_unchanged(self):
        s = "x" * js.CAP
        self.assertEqual(js.trim(s), s)

    def test_over_cap_truncated_with_marker(self):
        out = js.trim("x" * (js.CAP + 1))
        self.assertTrue(out.startswith("x" * js.CAP))
        self.assertTrue(out.endswith("[... truncated for the sheet ...]"))
        self.assertEqual(len(out), js.CAP + len("\n[... truncated for the sheet ...]"))

    def test_empty(self):
        self.assertEqual(js.trim(""), "")


class Hash(unittest.TestCase):
    def test_stable_and_separator_sensitive(self):
        self.assertEqual(js.h("a", "b"), js.h("a", "b"))
        self.assertNotEqual(js.h("a", "b"), js.h("ab"))
        self.assertEqual(len(js.h("a")), 64)


class AlarmCounts(unittest.TestCase):
    def test_matching_counts_return_firings(self):
        rows = synthetic()
        fires = [r for r in rows if r["alarm_fires"]]
        want = {"alarms": len(fires), "true_alarms": sum(r["top_is_judged_primary"] for r in fires)}
        self.assertEqual(js.check_alarm_counts(rows, want), fires)

    def test_wrong_alarm_total_refused(self):
        rows = synthetic()
        with self.assertRaises(ValueError):
            js.check_alarm_counts(rows, {"alarms": 47, "true_alarms": 43})

    def test_wrong_true_alarm_total_refused(self):
        rows = synthetic()
        n = sum(r["alarm_fires"] for r in rows)
        with self.assertRaises(ValueError):
            js.check_alarm_counts(rows, {"alarms": n, "true_alarms": 0})

    def test_primary_role_firings_never_count_as_true(self):
        rows = [row("c", 1, role="primary", fires=True, judged=True)]
        self.assertEqual(js.check_alarm_counts(rows, {"alarms": 1, "true_alarms": 0}), rows)

    def test_no_rows(self):
        self.assertEqual(js.check_alarm_counts([], {"alarms": 0, "true_alarms": 0}), [])


class Controls(unittest.TestCase):
    def test_excludes_abstained_firing_primary_and_no_candidate(self):
        rows = [row("c", 1, abstained=True), row("c", 2, fires=True), row("c", 3, role="primary"),
                row("c", 4, top=None), row("c", 5)]
        self.assertEqual([r["issue"] for r in js.select_controls(rows)], ["5"])

    def test_takes_at_most_n_and_is_deterministic(self):
        rows = synthetic()
        a = js.select_controls(rows)
        self.assertEqual(len(a), js.CONTROLS)
        self.assertEqual(a, js.select_controls(list(reversed(rows))))

    def test_fewer_than_n_returns_all(self):
        self.assertEqual(len(js.select_controls(synthetic(n_quiet=5))), 5)

    def test_seed_changes_selection(self):
        rows = synthetic()
        self.assertNotEqual([r["issue"] for r in js.select_controls(rows)],
                            [r["issue"] for r in js.select_controls(rows, seed="other")])

    def test_empty(self):
        self.assertEqual(js.select_controls([]), [])


class Items(unittest.TestCase):
    def setUp(self):
        rows = synthetic()
        self.alarms = [r for r in rows if r["alarm_fires"]]
        self.controls = js.select_controls(rows)
        self.items = js.number_items(self.alarms, self.controls)

    def test_counts_kinds_and_ids(self):
        self.assertEqual(len(self.items), 96)
        self.assertEqual(sum(i["kind"] == "alarm" for i in self.items), 48)
        self.assertEqual(sum(i["kind"] == "control" for i in self.items), 48)
        self.assertEqual([i["item"] for i in self.items], [f"I{n:03d}" for n in range(1, 97)])

    def test_order_does_not_group_by_kind(self):
        kinds = "".join(i["kind"][0] for i in self.items)
        self.assertNotIn("a" * 20, kinds)
        self.assertNotIn("c" * 20, kinds)

    def test_inputs_not_mutated(self):
        self.assertNotIn("kind", self.alarms[0])

    def test_empty(self):
        self.assertEqual(js.number_items([], []), [])


class Batches(unittest.TestCase):
    def setUp(self):
        rows = synthetic()
        self.items = js.number_items([r for r in rows if r["alarm_fires"]], js.select_controls(rows))
        self.anchors = js.pick_anchors(self.items)
        self.batches = js.split_batches(self.items, self.anchors)

    def test_anchor_count_and_membership(self):
        self.assertEqual(len(self.anchors), js.ANCHORS)
        self.assertTrue(self.anchors <= {i["item"] for i in self.items})

    def test_batch_sizes_smallest_first(self):
        self.assertEqual([len(v) for v in self.batches.values()], [23, 23, 23, 24])
        self.assertEqual(list(self.batches), ["P1", "P2", "P3", "P4"])

    def test_partition_is_exact_and_excludes_anchors(self):
        flat = [x for v in self.batches.values() for x in v]
        self.assertEqual(len(flat), len(set(flat)))
        self.assertEqual(set(flat) | self.anchors, {i["item"] for i in self.items})
        self.assertFalse(set(flat) & self.anchors)

    def test_fewer_items_than_batches(self):
        items = self.items[:5]
        b = js.split_batches(items, set())
        self.assertEqual(sorted(len(v) for v in b.values()), [1, 1, 1, 2])

    def test_no_items(self):
        self.assertEqual(js.split_batches([], set()), {f"P{i}": [] for i in range(1, 5)})

    def test_more_anchors_than_items(self):
        self.assertEqual(len(js.pick_anchors(self.items[:2])), 2)

    def test_shuffled_rows_include_anchors_and_depend_on_slot(self):
        ids = self.batches["P1"]
        a = js.shuffled_rows("P1-A", ids, self.anchors)
        b = js.shuffled_rows("P1-B", ids, self.anchors)
        self.assertEqual(sorted(a), sorted(ids + sorted(self.anchors)))
        self.assertEqual(sorted(a), sorted(b))
        self.assertNotEqual(a, b)
        self.assertEqual(a, js.shuffled_rows("P1-A", ids, self.anchors))
        self.assertEqual(len(a), 26)


class Sheet(unittest.TestCase):
    def setUp(self):
        self.by_item = {"I001": {"_q": "first, with \"quotes\"\nand a newline", "_c": "second", "kind": "alarm", "alarm_fires": True},
                        "I002": {"_q": "q2", "_c": "", "kind": "control"}}

    def test_csv_carries_only_texts_and_blank_label_columns(self):
        csv_text, _, _ = js.render_sheet("P1-A", ["I002", "I001"], self.by_item)
        rows = list(csv.reader(io.StringIO(csv_text)))
        self.assertEqual(rows[0][0], "row")
        self.assertEqual(rows[1], ["R01", "q2", "", "", ""])
        self.assertEqual(rows[2], ["R02", "first, with \"quotes\"\nand a newline", "second", "", ""])
        self.assertTrue(all(len(r) == 5 for r in rows))

    def test_nothing_that_unblinds(self):
        csv_text, md, _ = js.render_sheet("P1-A", ["I001", "I002"], self.by_item)
        for word in ("alarm", "control", "I001", "I002", "contest"):
            self.assertNotIn(word, csv_text.replace("alarm_fires", ""))
            if word != "contest":  # the markdown intro says "the contest's published report"
                self.assertNotIn(word, md)

    def test_rowkey_maps_in_order(self):
        _, _, key = js.render_sheet("P1-A", ["I002", "I001"], self.by_item)
        self.assertEqual(key, {"R01": "I002", "R02": "I001"})

    def test_markdown_structure(self):
        _, md, _ = js.render_sheet("P3-B", ["I001"], self.by_item)
        self.assertTrue(md.startswith("# Duplicate judging sheet P3-B\n"))
        self.assertIn("## R01", md)
        self.assertIn("Label: yes / no / unclear", md)
        self.assertTrue(md.endswith("\n"))
        self.assertNotIn("\r", md)

    def test_empty_sheet_has_only_header(self):
        csv_text, _, key = js.render_sheet("P1-A", [], self.by_item)
        self.assertEqual(len(list(csv.reader(io.StringIO(csv_text)))), 1)
        self.assertEqual(key, {})

    def test_unknown_item_raises(self):
        with self.assertRaises(KeyError):
            js.render_sheet("P1-A", ["I999"], self.by_item)


if __name__ == "__main__":
    unittest.main()