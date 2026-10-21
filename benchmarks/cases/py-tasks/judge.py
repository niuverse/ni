import unittest,tempfile,json,subprocess,sys
from pathlib import Path
class Acceptance(unittest.TestCase):
    def test_cli(self):
        tasks=[{'id':2,'status':'done','title':'中文'},{'id':1,'status':'open','title':'x'}]
        with tempfile.TemporaryDirectory() as d:
            file=Path(d)/'tasks.json'; file.write_text(json.dumps(tasks)); before=file.read_bytes()
            def run(*flags): return subprocess.run([sys.executable,'cli.py',str(file),*flags],capture_output=True,text=True,timeout=5)
            self.assertEqual(run().stdout,'2 [done] 中文\n1 [open] x\n')
            self.assertEqual(json.loads(run('--json').stdout),tasks)
            self.assertEqual(json.loads(run('--json','--status','open').stdout),[tasks[1]])
            self.assertEqual(run('--status','done').stdout,'2 [done] 中文\n')
            self.assertNotEqual(run('--status','bad').returncode,0)
            self.assertEqual(file.read_bytes(),before)
            file.write_text('[]'); self.assertEqual(json.loads(run('--status','open','--json').stdout),[])
if __name__=='__main__': unittest.main()
