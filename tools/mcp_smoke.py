#!/usr/bin/env python3
"""Drive the production Noton tree through its MCP stdio interface."""
import json, subprocess, sys, tempfile, pathlib, threading, queue

binary = sys.argv[1] if len(sys.argv)>1 else 'target/debug/noton'
class Client:
    def __init__(self,path,extra_args=()):
        self.p=subprocess.Popen([binary,'--mcp','--library',str(path),*extra_args],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        self.seq=0;self.lines=queue.Queue();self.errors=[]
        def reader():
            for line in self.p.stdout:self.lines.put(line)
        def errors():
            for line in self.p.stderr:self.errors.append(line)
        threading.Thread(target=reader,daemon=True).start();threading.Thread(target=errors,daemon=True).start()
        self.request('initialize',{})
    def request(self,method,params):
        self.seq+=1;self.p.stdin.write(json.dumps(dict(jsonrpc='2.0',id=self.seq,method=method,params=params))+'\n');self.p.stdin.flush()
        response=json.loads(self.lines.get(timeout=30));assert response['id']==self.seq,response
        assert 'error' not in response,response
        return response['result']
    def call(self,name,args={}):
        response=self.request('tools/call',dict(name=name,arguments=args));payload=json.loads(response['content'][0]['text']);assert not response.get('isError'),payload
        return payload['data']
    def nodes(self):return self.call('snapshot_tree')['nodes']
    def click(self,label,role='Button'):
        nodes=[n for n in self.nodes() if n.get('label')==label and n['role']==role];assert nodes,(label,[(n.get('role'),n.get('label'))for n in self.nodes()])
        self.call('invoke_action',dict(node=nodes[0]['id'],action='click'))
    def insert(self,item):self.click('Insert');self.click(item,'MenuItem')
    def execute(self,name,**args):return self.call('execute',{'operation':{name:args}})
    def close(self):
        self.p.stdin.close();self.p.wait(timeout=30);assert self.p.returncode==0,''.join(self.errors)
    def kill(self):
        self.p.kill();self.p.wait(timeout=10)

def run():
    with tempfile.TemporaryDirectory() as d:
        path=pathlib.Path(d)/'smoke.noton';c=Client(path)
        try:
            c.click('New folder');c.click('New note');c.insert('Text block')
            nodes=c.nodes();editors=[n for n in nodes if n['role']=='MultilineTextInput'];assert editors,nodes
            c.execute('FocusNode',node=editors[0]['id']);c.execute('TypeText',node=editors[0]['id'],text='Bonjour Noton — durable text')
            c.click('Reading','RadioButton')
            nodes=c.nodes();assert any('Bonjour Noton' in str(n.get('value','')) for n in nodes),nodes
            c.click('Canvas','RadioButton');c.insert('Sketch');c.execute('InjectKey',key='s',command=True);c.close()
            c=Client(path);nodes=c.nodes();assert any('New note' in str(n) for n in nodes),nodes
            c.click('Reading','RadioButton');nodes=c.nodes();assert any('Bonjour Noton' in str(n.get('value','')) for n in nodes),nodes
            c.close()
            print('MCP: create folder/note, type rich text, switch projections, insert sketch, save, reopen: passed')
        finally:
            if c.p.poll() is None:c.kill()
if __name__=='__main__':run()
