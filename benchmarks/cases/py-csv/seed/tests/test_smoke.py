import unittest
from summary import total
class Smoke(unittest.TestCase):
    def test_total(self): self.assertEqual(total('amount\n1.2\n2.3\n'),'3.5')
