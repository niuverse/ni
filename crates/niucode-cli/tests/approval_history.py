"""Approval interrupts history without losing its selected record/scroll position."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import time
from terminal import Terminal


def run(binary):
    release_tool, release_answer, stop = threading.Event(), threading.Event(), threading.Event()
    class Provider(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass
        def do_POST(self):
            request=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            self.send_response(200)
            self.send_header('Content-Type','text/event-stream')
            self.end_headers()
            def send(delta, reason=None):
                self.wfile.write(('data: '+json.dumps({'choices':[{'index':0,'delta':delta,'finish_reason':reason}]})+'\n\n').encode())
                self.wfile.flush()
            try:
                if any(m.get('role')=='tool' for m in request['messages']):
                    release_answer.wait(10)
                    if not stop.is_set():
                        send({'content':'FINISHED\n'},'stop')
                else:
                    send({'content':'WAITING_FOR_TOOL\n'})
                    release_tool.wait(10)
                    if not stop.is_set():
                        send({'tool_calls':[{'index':0,'id':'write','type':'function','function':{
                            'name':'write','arguments':json.dumps({'path':'created.txt','content':'new\n'})}}]},'tool_calls')
                self.wfile.write(b'data: [DONE]\n\n')
            except (BrokenPipeError,ConnectionResetError):
                pass
    server=ThreadingHTTPServer(('127.0.0.1',0),Provider)
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    with tempfile.TemporaryDirectory(prefix='niucode-history-') as directory:
        root=Path(directory);(root/'work').mkdir()
        terminal=Terminal(binary,root,args=['--preset','openai','--model','local-fixture','--approval','ask'],
            env_extra={'OPENAI_API_KEY':'local-test-only','OPENAI_BASE_URL':f'http://127.0.0.1:{server.server_port}/v1'})
        try:
            terminal.wait_for(b'\x1b[?2004h')
            prompt='\n'.join(f'history line {i:02}' for i in range(50))
            os.write(terminal.master,b'\x1b[200~'+prompt.encode()+b'\x1b[201~\r')
            terminal.wait_for(b'WAITING_FOR_TOOL')
            terminal.transcript.clear();os.write(terminal.master,b'\x0f')
            terminal.wait_for(b'history line 00')
            terminal.transcript.clear();os.write(terminal.master,b'\x1b[6~')
            terminal.wait_for(b'history line 22')
            release_tool.set();terminal.wait_for(b'a allow / d deny')
            # A modified key is not an approval shortcut.
            os.write(terminal.master,b'\x01\x11')
            terminal.pump(.1)
            assert not (root/'work'/'created.txt').exists()
            terminal.transcript.clear();os.write(terminal.master,b'a')
            terminal.wait_for(b'history line 22')
            assert b'history line 00' not in terminal.transcript
            terminal.transcript.clear();os.write(terminal.master,b'q')
            terminal.wait_for(b'\x1b[?1049l')
            release_answer.set();terminal.wait_for(b'done')
            terminal.wait_for(b'\x1b[?2004h',terminal.transcript.index(b'done'))
            assert (root/'work'/'created.txt').read_text()=='new\n'
            os.write(terminal.master,b'\x04')
            deadline=time.monotonic()+5
            while terminal.child.poll() is None and time.monotonic()<deadline:
                terminal.pump(.05)
            assert terminal.child.poll() is not None, terminal.transcript[-2500:]
            assert terminal.child.returncode==0
            print('Approval preserved history selection/scroll; Ctrl-A / Ctrl-Q neither approved nor hid the request')
        finally:
            stop.set();release_tool.set();release_answer.set()
            terminal.close();server.shutdown();server.server_close();worker.join(timeout=2)


if __name__=='__main__':
    run(str(Path(sys.argv[1]).resolve()))
