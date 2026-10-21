import unittest, copy
from config import resolve
class Acceptance(unittest.TestCase):
    def test_falsey(self):
        for value in [False,0,'',[],{}]:
            self.assertEqual(resolve({'x':9},{'x':8},{'x':'7'},{'x':value}),{'x':value})
    def test_env_and_absent(self):
        self.assertEqual(resolve({'x':9},{'x':8},{'x':'0'},{'x':None}),{'x':0})
        self.assertEqual(resolve({'x':9},{'x':8},{'x':None},{}),{'x':8})
    def test_mutation(self):
        args=[{'x':1},{'y':2},{'z':'3'},{'q':False}]; before=copy.deepcopy(args)
        resolve(*args); self.assertEqual(args,before)
    def test_invalid(self):
        with self.assertRaises(ValueError): resolve({}, {}, {'x':'oops'}, {})
if __name__=='__main__': unittest.main()
