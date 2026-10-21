import unittest
from render import render_text
class Smoke(unittest.TestCase):
    def test_text(self): self.assertEqual(render_text([{'id':1,'status':'open','title':'hi'}]),'1 [open] hi')
