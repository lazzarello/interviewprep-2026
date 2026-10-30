import runpy
import unittest
from pathlib import Path


LRUCache = runpy.run_path(Path(__file__).with_name("lru-cache.py"))["LRUCache"]


class LRUCacheTests(unittest.TestCase):
    def assert_order(self, cache, expected):
        """Check least-to-most order, both link directions, and dictionary identity."""
        self.assertEqual(set(cache.cache), set(expected))
        self.assertLessEqual(len(cache.cache), cache.capacity)
        node = cache.start
        previous = None
        # A bounded walk also detects cycles without hanging the test.
        for key in expected:
            self.assertIsNotNone(node)
            self.assertEqual(node.key, key)
            self.assertIs(node, cache.cache[key])
            self.assertIs(node.prev, previous)
            previous, node = node, node.next
        self.assertIsNone(node)
        self.assertIs(cache.end, previous)

    def test_missing_key_does_not_change_order(self):
        cache = LRUCache(2)
        self.assertIsNone(cache.get("missing"))
        self.assert_order(cache, [])
        cache.put("A", 10)
        cache.put("B", 20)
        self.assertIsNone(cache.get("missing"))
        self.assert_order(cache, ["A", "B"])

    def test_get_moves_first_middle_or_last_to_end(self):
        cases = [
            ("A", 10, ["B", "C", "D", "A"]),
            ("B", 20, ["A", "C", "D", "B"]),
            ("D", 40, ["A", "B", "C", "D"]),
        ]
        for key, value, expected in cases:
            with self.subTest(key=key):
                cache = LRUCache(4)
                for name, number in zip("ABCD", [10, 20, 30, 40]):
                    cache.put(name, number)
                self.assert_order(cache, ["A", "B", "C", "D"])
                self.assertEqual(cache.get(key), value)
                self.assert_order(cache, expected)

    def test_get_first_of_two_nodes(self):
        cache = LRUCache(2)
        cache.put("A", 10)
        cache.put("B", 20)
        self.assertEqual(cache.get("A"), 10)
        self.assert_order(cache, ["B", "A"])

    def test_duplicate_put_reuses_node_and_promotes_it(self):
        for key, expected in [
            ("A", ["B", "C", "A"]),
            ("B", ["A", "C", "B"]),
            ("C", ["A", "B", "C"]),
        ]:
            with self.subTest(key=key):
                cache = LRUCache(3)
                for name in "ABC":
                    cache.put(name, 10)
                original = cache.cache[key]
                cache.put(key, 99)
                self.assertIs(cache.cache[key], original)
                self.assertEqual(original.value, 99)
                self.assert_order(cache, expected)
                self.assertEqual(cache.get(key), 99)
                self.assert_order(cache, expected)

    def test_eviction_respects_reads_and_duplicate_writes(self):
        cache = LRUCache(2)
        cache.put("A", 10)
        cache.put("B", 20)
        self.assertEqual(cache.get("A"), 10)
        cache.put("C", 30)
        self.assertIsNone(cache.get("B"))
        self.assert_order(cache, ["A", "C"])
        cache.put("A", 99)
        self.assert_order(cache, ["C", "A"])
        cache.put("D", 40)
        self.assertIsNone(cache.get("C"))
        self.assert_order(cache, ["A", "D"])
        self.assertEqual(cache.get("A"), 99)
        self.assert_order(cache, ["D", "A"])

    def test_falsy_keys_and_values_are_valid_entries(self):
        cache = LRUCache(2)
        cache.put(0, 0)
        cache.put("", None)
        self.assertEqual(cache.get(0), 0)
        self.assert_order(cache, ["", 0])
        self.assertIsNone(cache.get(""))
        self.assert_order(cache, [0, ""])
        cache.put("", 7)
        cache.put(0, 8)
        self.assert_order(cache, ["", 0])
        self.assertEqual(cache.get(0), 8)
        self.assertEqual(cache.get(""), 7)
        self.assert_order(cache, [0, ""])

    def test_capacity_one_reads_updates_and_replacement(self):
        cache = LRUCache(1)
        cache.put("A", 10)
        for _ in range(2):
            self.assertEqual(cache.get("A"), 10)
            self.assert_order(cache, ["A"])
        cache.put("A", 99)
        self.assertEqual(cache.get("A"), 99)
        self.assert_order(cache, ["A"])
        cache.put("B", 20)
        self.assertIsNone(cache.get("A"))
        self.assertEqual(cache.get("B"), 20)
        self.assert_order(cache, ["B"])

    def test_capacity_zero_never_retains_entries(self):
        cache = LRUCache(0)
        for key in ["A", "B", "A"]:
            cache.put(key, 10)
            self.assertIsNone(cache.get(key))
            self.assert_order(cache, [])


if __name__ == "__main__":
    unittest.main()
