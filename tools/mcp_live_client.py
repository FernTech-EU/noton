"""Small MCP client for an explicitly selected Noton process; saves audit evidence."""
import base64
import json
import pathlib
import queue
import subprocess
import threading


class LiveClient:
    def __init__(self, pid, output):
        self.output = pathlib.Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.process = subprocess.Popen(
            ['teksilo-automation-mcp', '--attach-pid', str(pid)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        self.lines = queue.Queue()
        self.errors = []
        self.seq = 0
        self.window = None
        def read():
            for line in self.process.stdout:
                self.lines.put(line)
        def errors():
            for line in self.process.stderr:
                self.errors.append(line)
        threading.Thread(target=read, daemon=True).start()
        threading.Thread(target=errors, daemon=True).start()
        self.request('initialize', {'protocolVersion': '2024-11-05', 'capabilities': {},
                                   'clientInfo': {'name': 'noton-audit', 'version': '1'}})
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'method': 'notifications/initialized'})+'\n')
        self.process.stdin.flush()

    def request(self, method, params):
        self.seq += 1
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': self.seq, 'method': method, 'params': params})+'\n')
        self.process.stdin.flush()
        while True:
            reply = json.loads(self.lines.get(timeout=40))
            if reply.get('id') == self.seq:
                if 'error' in reply:
                    raise RuntimeError(reply['error'])
                return reply['result']

    def raw(self, name, args=None):
        args = dict(args or {})
        if self.window is not None:
            args.setdefault('window_id', self.window)
        result = self.request('tools/call', {'name': name, 'arguments': args})
        # Screenshots are saved separately rather than duplicating base64 in the transcript.
        with (self.output/'transcript.jsonl').open('a') as stream:
            stream.write(json.dumps({'tool': name, 'arguments': args, 'result': {
                **result, 'content': [b if b['type'] != 'image' else {'type': 'image', 'saved_separately': True}
                                     for b in result.get('content', [])]}})+'\n')
        if result.get('isError'):
            raise RuntimeError(result)
        return result

    def call(self, name, args=None):
        result = self.raw(name, args)
        return json.loads(next(b['text'] for b in result['content'] if b['type'] == 'text'))

    def nodes(self):
        self.call('settle')
        return self.call('snapshot_tree')['nodes']

    def find(self, label, role='Button'):
        found = [n for n in self.nodes() if n.get('label') == label and n['role'] == role]
        assert len(found) == 1, (label, role, found)
        return found[0]

    def click(self, label, role='Button'):
        self.call('invoke_action', {'node': self.find(label, role)['id'], 'action': 'click'})

    def capture(self, name):
        self.call('settle')
        result = self.raw('screenshot')
        images = [b for b in result['content'] if b['type'] == 'image']
        assert len(images) == 1, result
        (self.output/f'{name}.png').write_bytes(base64.b64decode(images[0]['data']))
        for tool in ['snapshot_tree', 'layout_tree']:
            (self.output/f'{name}-{tool}.json').write_text(json.dumps(self.call(tool), indent=2))

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=5)
