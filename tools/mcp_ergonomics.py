#!/usr/bin/env python3
"""First-writing, navigation and search regressions through the production UI."""
import json
import pathlib
import sys
import tempfile
import os
import subprocess
import time
from mcp_live_client import LiveClient
from mcp_stories import action, node, set_field, note_menu, save, records


class Client(LiveClient):
    """Run mount/focus callbacks through the native event loop."""
    def __init__(self, path, args, output):
        self.log = (output/'app.log').open('w')
        env = dict(os.environ, XDG_CONFIG_HOME=str(output/'config'), XDG_DATA_HOME=str(output/'data'))
        self.p = subprocess.Popen([sys.argv[1], '--library', str(path), '--locale', 'en-US', *args],
                                  env=env, stdout=self.log, stderr=self.log)
        runtime = pathlib.Path(os.environ.get('XDG_RUNTIME_DIR', '/tmp'))/'teksilo-automation'/f'{self.p.pid}.json'
        deadline = time.monotonic()+20
        while not runtime.exists():
            assert self.p.poll() is None, 'App exited; see private app.log'
            assert time.monotonic() < deadline, 'Automation bridge timed out'
            time.sleep(.1)
        super().__init__(self.p.pid, output)

    def execute(self, name, **args):
        names = {'InjectKey': 'inject_key', 'FocusNode': 'focus_node', 'TypeText': 'type_text', 'SetValue': 'set_value'}
        return self.call(names[name], args)

    def insert(self, item):
        self.click('Insert')
        self.click(item, 'MenuItem')

    def close(self):
        self.call('inject_key', {'key': 'q', 'command': True})
        self.p.wait(timeout=15)
        super().close()
        self.log.close()

    def kill(self):
        self.p.terminate()
        self.p.wait(timeout=10)
        super().close()
        self.log.close()


def focused_editor(c):
    editors = [n for n in c.nodes() if n['role'] == 'MultilineTextInput' and n.get('focused')]
    assert len(editors) == 1, ('Expected keyboard focus in the new/matching editor', editors)
    return editors[0]


def rename(c, title):
    note_menu(c, 'Rename note')
    set_field(c, 'Title', title, True)


def quick_write(c, path):
    action(c, 'Start a note')
    focused_editor(c)
    # No explicit FocusNode: the action itself must make the keyboard usable.
    c.execute('InjectKey', key='a', text='a')
    save(c)
    assert 'a' in records(path, 'block')[0]['content']['djot']
    assert not any(n.get('label') == 'Editing' for n in c.nodes())
    c.capture('immediate-writing')
    c.execute('InjectKey', key='n', command=True)
    focused_editor(c)
    save(c)
    assert len(records(path, 'note')) == 2
    assert len(records(path, 'block')) == 2


def navigation(c, path):
    action(c, 'Start a note')
    focused_editor(c)
    action(c, 'Navigation')
    assert not any(n['role'] == 'ListBoxOption' for n in c.nodes())
    assert any(n['role'] == 'MultilineTextInput' for n in c.nodes())
    c.capture('navigation-hidden')
    c.execute('InjectKey', key='\\', command=True, shift=True)
    assert any(n['role'] == 'ListBoxOption' for n in c.nodes())


def blank_canvas(c, path):
    action(c, 'Start a note')
    focused_editor(c)
    c.call('inject_pointer', {'action': 'double_click', 'x': 1000, 'y': 450})
    focused_editor(c)
    c.execute('InjectKey', key='b', text='b')
    save(c)
    blocks = records(path, 'block')
    assert len(blocks) == 2, blocks
    assert any('b' in b['content']['djot'] for b in blocks), blocks
    c.capture('blank-canvas-writing')


def empty_note(c, path):
    action(c, 'New folder')
    action(c, 'New note')
    c.execute('InjectKey', key='Enter')
    action(c, 'Start writing')
    focused_editor(c)
    c.execute('InjectKey', key='e', text='e')
    save(c)
    assert 'e' in records(path, 'block')[0]['content']['djot']


def search(c, path):
    action(c, 'Start a note')
    rename(c, 'Original')
    action(c, 'Start a note')
    rename(c, 'Destination')
    c.insert('Text block')
    editors = [n for n in c.nodes() if n['role'] == 'MultilineTextInput']
    target = editors[-1]['id']
    c.execute('FocusNode', node=target)
    c.execute('TypeText', node=target, text='A long introduction.\n\n☕ Café and café tomorrow.')
    save(c)
    action(c, 'Original', 'ListBoxOption')
    set_field(c, 'Search library', 'cafe')
    assert node(c, 'Rename note')['value'] == 'Original', 'Typing a query changed the open note'
    results = [n for n in c.nodes() if n['role'] == 'ListBoxOption' and n.get('label') == 'Destination']
    assert len(results) == 1
    assert any('2 matches' in str(n) for n in c.nodes()), 'Missing occurrence count'
    assert any('Café' in str(n) for n in c.nodes()), 'Missing match excerpt'
    action(c, 'Destination', 'ListBoxOption')
    editor = focused_editor(c)
    text = editor.get('text', {})
    assert 'Café' in text.get('document_text', ''), editor
    # The selection must name the actual occurrence, not the first block.
    c.capture('matching-text')
    c.execute('InjectKey', key='x', text='x')
    actual = focused_editor(c)['text']['document_text']
    assert actual == text['document_text'].replace('Café', 'x', 1), actual
    c.execute('InjectKey', key='z', command=True)
    set_field(c, 'Search library', 'no-such-occurrence')
    assert not any(n['role'] == 'ListBoxOption' and n.get('label') in ('Original', 'Destination') for n in c.nodes())
    c.execute('InjectKey', key='Escape')
    assert node(c, 'Rename note')['value'] == 'Original', 'Dismissing search lost the previous note'


def main():
    out = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else 'target/ergonomics-results')
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for name, scenario, args in [
        ('quick-write', quick_write, ()),
        ('quick-write-narrow', quick_write, ('--window-size', '600x700')),
        ('navigation-toggle', navigation, ()),
        ('blank-canvas', blank_canvas, ()),
        ('empty-note', empty_note, ()),
        ('match-aware-search', search, ()),
    ]:
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d)/'ergonomics.noton'
            directory = out/name
            directory.mkdir(parents=True, exist_ok=True)
            c = Client(path, args, directory)
            try:
                scenario(c, path)
                c.close()
                result = {'scenario': name, 'status': 'passed'}
            except Exception as e:
                import traceback
                traceback.print_exc()
                result = {'scenario': name, 'status': 'failed', 'error': str(e)}
                (out/f'{name}-tree.json').write_text(json.dumps(c.nodes(), indent=2))
            finally:
                if c.p.poll() is None:
                    c.kill()
            results.append(result)
            print(result, flush=True)
    (out/'results.json').write_text(json.dumps(results, indent=2))
    return int(any(r['status'] == 'failed' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
