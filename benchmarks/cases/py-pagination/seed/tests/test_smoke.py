import unittest
from client import collect
class Smoke(unittest.TestCase):
    def test_single(self): self.assertEqual(collect(lambda _: {'items':[{'id':1}],'next':None},'a'),[{'id':1}])
