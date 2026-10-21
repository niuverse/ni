import unittest
from formatter import format_record
class Acceptance(unittest.TestCase):
    def test_format(self):
        data={'z':2,'a':'中文'}
        self.assertEqual(format_record(data),'{"a":"中文","z":2}')
        self.assertEqual(data,{'z':2,'a':'中文'})
if __name__=='__main__': unittest.main()

