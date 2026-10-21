import unittest,json
from formatter import format_record
class Smoke(unittest.TestCase):
    def test_roundtrip(self): self.assertEqual(json.loads(format_record({'a':1})),{'a':1})

