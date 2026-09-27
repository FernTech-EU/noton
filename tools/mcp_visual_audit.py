#!/usr/bin/env python3
"""Launch isolated native Noton instances and capture real MCP screenshots.

Never attaches to an arbitrary/latest app. Only the child processes created here
are driven or closed. Requires access to the desktop compositor and MCP socket.
"""
import json
import os
import pathlib
import subprocess
import sys
import time

from mcp_live_client import LiveClient


def insert(client, item):
    client.click('Insert')
    client.click(item, 'MenuItem')


def run_window(binary, output, size, reopen=False):
    directory = output / size
    directory.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, XDG_CONFIG_HOME=str(directory/'config'), XDG_DATA_HOME=str(directory/'data'))
    with (directory/'app.log').open('w') as log:
        process = subprocess.Popen([binary, '--library', str(directory/'audit.noton'), '--locale', 'en-US', '--window-size', size], env=env, stdout=log, stderr=log)
        client = None
        try:
            runtime = pathlib.Path(os.environ.get('XDG_RUNTIME_DIR', '/tmp'))/'teksilo-automation'/f'{process.pid}.json'
            deadline = time.monotonic()+20
            while not runtime.exists():
                assert process.poll() is None, f'Noton exited with {process.returncode}; see private app.log'
                assert time.monotonic() < deadline, 'Timed out waiting for automation bridge'
                time.sleep(.1)
            client = LiveClient(process.pid, directory)
            windows = client.call('list_windows')
            (directory/'windows.json').write_text(json.dumps(windows, indent=2))
            if reopen:
                client.capture('12-reopened')
                if size == '1280x850':
                    client.click('Planning', 'ListBoxOption')
                    client.capture('13-table-reopened-canvas')
                    client.click('Reading', 'RadioButton')
                    client.capture('14-table-reopened-reading')
                    cells = [n for n in client.nodes() if n['role'] == 'Cell']
                    assert len(cells) == 9 and all('bounds' in n for n in cells), 'Reopened table cells lack geometry'
                return
            client.capture('01-empty')
            client.click('New folder')
            if size == '600x700':
                client.click('New folder', 'ListBoxOption')
            client.click('New note')
            client.call('set_value', {'node':client.find('Title', 'TextInput')['id'], 'value':'Réunion projet'})
            insert(client, 'Text block')
            editor = next(n for n in client.nodes() if n['role']=='MultilineTextInput')
            client.call('type_text', {'node':editor['id'], 'text':'Réunion de projet'})
            client.call('inject_key', {'key':'Enter'})
            client.call('type_text', {'node':editor['id'], 'text':'Décisions et prochaines étapes.'})
            client.capture('02-text-canvas')
            if size == '600x700':
                client.click('Reading', 'RadioButton')
                client.capture('03-reading')
                client.click('Back')
                client.capture('04-notes')
                return
            insert(client, 'Sketch')
            client.capture('03-sketch-canvas')
            # Sketches draw in drawing mode, entered from the block's ⋮ menu.
            menus = [n for n in client.nodes() if n['role'] == 'Button' and n.get('label') == 'Block actions']
            client.call('invoke_action', {'node': menus[-1]['id'], 'action': 'click'})
            client.click('Draw', 'MenuItem')
            surface = client.find('Sketch block', 'Image')['bounds']
            for phase, dx, dy in [('down',0,0),('move',80,20),('move',160,0),('up',180,30)]:
                client.call('inject_pointer', {'action':phase,'x':surface['x']+30+dx,'y':surface['y']+30+dy,'kind':'pen','pressure':.7})
            client.capture('04-ink-canvas')
            client.click('Done drawing')
            client.click('Reading', 'RadioButton')
            client.capture('05-reading')
            client.click('New note')
            client.call('set_value', {'node':client.find('Title', 'TextInput')['id'], 'value':'Planning'})
            insert(client, 'Text block')
            editor = next(n for n in client.nodes() if n['role']=='MultilineTextInput')
            client.call('focus_node', {'node':editor['id']})
            client.click('Table')
            client.click('Insert 3 × 3 table', 'MenuItem')
            client.call('type_text', {'node':editor['id'], 'text':'Cellule — café'})
            client.capture('06-table-reading')
            client.click('Canvas', 'RadioButton')
            client.capture('07-table-canvas')
            client.call('inject_key', {'key':'s', 'command':True})
            before_text = next(n for n in client.nodes() if n['role'] == 'MultilineTextInput').get('text', {}).get('document_text')
            client.call('inject_key', {'key':',', 'command':True})
            dialog = client.find('Settings', 'Dialog')
            assert any(n.get('focused') and n.get('label') == 'Language' for n in client.nodes()), 'Initial Settings focus is not on language'
            client.capture('08-settings-modal')
            # Tab must remain inside the dialog, and document history must stay suspended.
            nodes = {n['id']: n for n in client.nodes()}
            descendants = set()
            pending = [dialog['id']]
            while pending:
                current = pending.pop()
                if current in descendants:
                    continue
                descendants.add(current)
                pending.extend(nodes.get(current, {}).get('children', []))
            for _ in range(12):
                client.call('inject_key', {'key':'Tab'})
                focused = [n['id'] for n in client.nodes() if n.get('focused')]
                assert focused and all(n in descendants for n in focused), ('Focus escaped Settings', focused)
            client.call('inject_key', {'key':'z', 'command':True})
            client.click('Français', 'RadioButton')
            client.capture('09-settings-fr')
            client.click('Sombre', 'RadioButton')
            client.capture('10-settings-dark')
            client.call('inject_key', {'key':'Escape'})
            assert not any(n['role'] == 'Dialog' for n in client.nodes())
            after_text = next(n for n in client.nodes() if n['role'] == 'MultilineTextInput').get('text', {}).get('document_text')
            assert after_text == before_text, 'Settings shortcut changed document history'
            client.capture('11-main-fr-dark')
            client.call('inject_key', {'key':',', 'command':True})
            client.click('Terminé')
            assert not any(n['role'] == 'Dialog' for n in client.nodes())
            client.call('inject_key', {'key':',', 'command':True})
            client.click('Fermer')
            assert not any(n['role'] == 'Dialog' for n in client.nodes())
        finally:
            if client is not None:
                try:
                    # Close through Noton's normal save barrier, scoped to main window.
                    client.call('inject_key', {'key':'q', 'command':True})
                    process.wait(timeout=5)
                except Exception:
                    process.terminate()
                client.close()
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


def main():
    binary = str(pathlib.Path(sys.argv[1] if len(sys.argv)>1 else 'target/debug/noton').resolve())
    output = pathlib.Path(sys.argv[2] if len(sys.argv)>2 else '/tmp/noton-visual-audit').resolve()
    failures = []
    for size in ['1280x850', '600x700']:
        try:
            run_window(binary, output, size)
            run_window(binary, output, size, reopen=True)
            print(f'CAPTURED {size}', flush=True)
        except Exception as error:
            failures.append({'size':size,'error':str(error)})
            print(f'FAILED {size}: {error}', flush=True)
    (output/'failures.json').write_text(json.dumps(failures, indent=2))
    return bool(failures)


if __name__ == '__main__':
    raise SystemExit(main())
