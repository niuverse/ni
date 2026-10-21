import unittest
import summary
class Acceptance(unittest.TestCase):
    def test_exact(self):
        r=summary.summarize('team,amount\n"a,b",0.1\nz,-2\n"a,b",0.2\n','team','amount')
        self.assertEqual(r,{'a,b':'0.3','z':'-2'}); self.assertEqual(list(r),['a,b','z'])
    def test_missing(self):
        text='team,amount\na,\nb,2\nc\n'
        with self.assertRaises(ValueError): summary.summarize(text,'team','amount')
        self.assertEqual(summary.summarize(text,'team','amount','skip'),{'b':'2'})
        self.assertEqual(summary.summarize(text,'team','amount','zero'),{'a':'0','b':'2','c':'0'})
    def test_invalid(self):
        for value in ['nope','NaN','Infinity']:
            with self.assertRaises(ValueError): summary.summarize(f'team,amount\na,{value}\n','team','amount')
        with self.assertRaises(ValueError): summary.summarize('a,b\n','team','amount')
        with self.assertRaises(ValueError): summary.summarize('team,amount\n','team','amount','other')
        self.assertEqual(summary.total('amount\n2.5\n'),'2.5')
if __name__=='__main__': unittest.main()
