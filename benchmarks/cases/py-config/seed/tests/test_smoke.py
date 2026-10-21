import unittest
from config import resolve
class Smoke(unittest.TestCase):
    def test_precedence(self): self.assertEqual(resolve({'x':1},{'x':2},{'x':'3'},{'x':4}),{'x':4})
