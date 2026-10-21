import unittest,copy,json,threading
from http.server import BaseHTTPRequestHandler,HTTPServer
from client import collect
from transport import fetch
class Acceptance(unittest.TestCase):
    def test_empty_and_duplicates(self):
        pages={'a':{'items':[{'id':2,'v':'first'}],'next':'b'},'b':{'items':[],'next':'c'},'c':{'items':[{'id':2,'v':'last'},{'id':1}],'next':None}}
        before=copy.deepcopy(pages); calls=[]
        def get(url): calls.append(url); return pages[url]
        self.assertEqual(collect(get,'a'),[{'id':2,'v':'first'},{'id':1}])
        self.assertEqual(calls,['a','b','c']); self.assertEqual(pages,before)
    def test_cycle(self):
        calls=[]
        def get(url):
            calls.append(url)
            if len(calls)>2: raise RuntimeError('refetched cycle')
            return {'items':[{'id':1}],'next':url}
        with self.assertRaises(ValueError): collect(get,'a')
        self.assertEqual(calls,['a'])
    def test_error(self):
        def get(_): raise OSError('offline')
        with self.assertRaises(OSError): collect(get,'a')
    def test_http(self):
        class Handler(BaseHTTPRequestHandler):
            def log_message(self,*args): pass
            def do_GET(self):
                data={'items':[],'next':f'http://127.0.0.1:{self.server.server_port}/last'} if self.path=='/first' else {'items':[{'id':'ok'}],'next':None}
                self.send_response(200); self.end_headers(); self.wfile.write(json.dumps(data).encode())
        with HTTPServer(('127.0.0.1',0),Handler) as server:
            thread=threading.Thread(target=server.serve_forever,daemon=True); thread.start()
            try: self.assertEqual(collect(fetch,f'http://127.0.0.1:{server.server_port}/first'),[{'id':'ok'}])
            finally: server.shutdown(); thread.join()
if __name__=='__main__': unittest.main()
