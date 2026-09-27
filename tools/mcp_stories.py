#!/usr/bin/env python3
"""Executable user-flow checks against Noton's production MCP tree and SQLite file.

Each scenario gets a fresh library. Failures are collected, never skipped or
converted to passing results. The report covers only the named checks below.
"""
import json
import pathlib
import sqlite3
import sys
import subprocess
import tempfile
import time

from mcp_smoke import Client


def node(c, label=None, role=None):
    found = [n for n in c.nodes() if (label is None or n.get('label') == label)
             and (role is None or n['role'] == role)]
    assert len(found) == 1, (label, role, found)
    return found[0]


def click(c, target):
    c.call('invoke_action', {'node': target['id'], 'action': 'click'})


def action(c, label, role='Button'):
    click(c, node(c, label, role))


def set_field(c, label, text, submit=False):
    target = node(c, label, 'TextInput')['id']
    c.execute('FocusNode', node=target)
    c.execute('SetValue', node=target, value=text)
    if submit:
        c.execute('InjectKey', key='Enter')


def starting(c, prefix, role):
    """The one node of `role` whose label starts with `prefix` (named undo)."""
    found = [n for n in c.nodes() if n['role'] == role and (n.get('label') or '').startswith(prefix)]
    assert len(found) == 1, (prefix, role, found)
    return found[0]


def menu(c, section, item):
    action(c, 'Menu')
    c.execute('Expand', node=node(c, section, 'MenuItem')['id'])
    click(c, starting(c, item, 'MenuItem'))


def insert(c, item):
    action(c, 'Insert')
    action(c, item, 'MenuItem')


def note_menu(c, item):
    action(c, 'Note actions')
    action(c, item, 'MenuItem')


def block_menu(c, item, index=0):
    buttons = [n for n in c.nodes() if n['role'] == 'Button' and n.get('label') == 'Block actions']
    click(c, buttons[index])
    action(c, item, 'MenuItem')


def seed(c, block='Text block'):
    for label in ['New folder', 'New note']:
        action(c, label)
    if block:
        insert(c, block)


def editor(c):
    return node(c, role='MultilineTextInput')['id']


def type_text(c, text):
    target = editor(c)
    c.execute('FocusNode', node=target)
    c.execute('TypeText', node=target, text=text)


def records(path, kind):
    with sqlite3.connect(path) as db:
        return [json.loads(row[0]) for row in db.execute('select payload from records where kind=? order by ordinal', (kind,))]


def save(c):
    c.execute('InjectKey', key='s', command=True)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        nodes = c.nodes()
        if any(n['role'] == 'Status' and (n.get('label') or '').startswith('Saved') for n in nodes):
            return
        time.sleep(.03)
    raise AssertionError('Save was not acknowledged within 5 seconds')


def navigation(c, path):
    for folder in ['Alpha', 'Beta']:
        action(c, 'New folder')
        # A new folder opens in inline rename; a new note, in title rename.
        set_field(c, 'Rename', folder, True)
        for suffix in ['1', '2']:
            action(c, 'New note')
            set_field(c, 'Title', folder+suffix, True)
            save(c)
    action(c, 'Alpha', 'ListBoxOption')
    labels = {n.get('label') for n in c.nodes() if n['role'] == 'ListBoxOption'}
    assert labels == {'Alpha', 'Beta', 'Alpha1', 'Alpha2'}, labels
    action(c, 'Alpha2', 'ListBoxOption')
    assert node(c, 'Rename note', 'Button').get('value') == 'Alpha2'
    action(c, 'Note actions')
    c.execute('Expand', node=node(c, 'Move to folder', 'MenuItem')['id'])
    action(c, 'Beta', 'MenuItem')
    assert not any(n.get('label') == 'Alpha2' for n in c.nodes()), 'Alpha2 stayed in Alpha'
    menu(c, 'Edit', 'Undo move note in folders')
    assert any(n.get('label') == 'Alpha2' for n in c.nodes())
    save(c)
    assert len(records(path, 'folder')) == 2
    assert len(records(path, 'note')) == 4


def reorder(c, path):
    action(c, 'New folder')
    set_field(c, 'Rename', 'Ordre', True)
    for title in ['Premier', 'Second']:
        action(c, 'New note')
        set_field(c, 'Title', title, True)
    # Alt+↓ on the selected row is the keyboard equivalent of dragging it.
    action(c, 'Premier', 'ListBoxOption')
    nodes = c.nodes()
    parent = {child: n for n in nodes for child in n.get('children', [])}
    here = node(c, 'Premier', 'ListBoxOption')
    while here['role'] != 'ListBox':
        here = parent[here['id']]
    c.execute('FocusNode', node=here['id'])
    c.execute('InjectKey', key='ArrowDown', alt=True)
    save(c)
    assert [n['title'] for n in records(path, 'note')] == ['Second', 'Premier'], records(path, 'note')
    menu(c, 'Edit', 'Undo reorder notes in folders')
    save(c)
    assert [n['title'] for n in records(path, 'note')] == ['Premier', 'Second'], records(path, 'note')


def text_roundtrip(c, path):
    seed(c)
    type_text(c, 'Été 📝 café')
    c.execute('InjectKey', key='Enter')
    c.execute('TypeText', node=editor(c), text='Deuxième ligne')
    action(c, 'Reading', 'RadioButton')
    save(c)
    payload = records(path, 'block')[0]['content']['djot']
    assert 'Été 📝 café' in payload and '\n' in payload and 'Deuxième ligne' in payload, payload


def formatting(c, path):
    seed(c)
    type_text(c, 'Texte accentué')
    c.execute('InjectKey', key='a', command=True)
    bold = node(c, 'Bold')
    assert bold['role'] in ('Button', 'ToggleButton'), bold
    click(c, bold)
    save(c)
    payload = records(path, 'block')[0]['content']['djot']
    assert '*Texte accentué*' in payload, payload
    menu(c, 'Edit', 'Undo typing')
    save(c)
    assert '*Texte accentué*' not in records(path, 'block')[0]['content']['djot']


def movement(c, path):
    seed(c)
    type_text(c, 'Do not rewind this text')
    save(c)
    original = records(path, 'block')[0]
    # Keyboard alternatives to dragging: Escape hands the keyboard from the
    # text to its card; Shift+→ moves one 24 px grid step, Ctrl+Shift+→ widens
    # by one. (The headless host presents no modal, so Position and size… is
    # checked in the live-window audit.)
    c.execute('InjectKey', key='Escape')
    c.execute('InjectKey', key='ArrowRight', shift=True)
    c.execute('InjectKey', key='ArrowRight', shift=True, command=True)
    save(c)
    current = records(path, 'block')[0]
    assert current['geometry']['x'] == original['geometry']['x'] + 24, current
    assert current['geometry']['width'] == original['geometry']['width'] + 24, current
    menu(c, 'Edit', 'Undo move block on this page')
    menu(c, 'Edit', 'Undo move block on this page')
    save(c)
    current = records(path, 'block')[0]
    assert current['geometry'] == original['geometry']
    assert current['content'] == original['content']


def trash_restore(c, path):
    seed(c)
    type_text(c, 'Recoverable')
    note_menu(c, 'Move to trash')
    assert not any(n.get('label') == 'New note' and n['role'] == 'ListBoxOption' for n in c.nodes())
    action(c, 'Trash')
    action(c, 'New note', 'ListBoxOption')
    note_menu(c, 'Restore note')
    action(c, 'Trash')
    save(c)
    assert not records(path, 'note')[0]['trashed']
    assert 'Recoverable' in records(path, 'block')[0]['content']['djot']


def search(c, path):
    seed(c)
    type_text(c, 'Café réunion')
    save(c)
    set_field(c, 'Search library', 'cafe')
    assert any(n['role'] == 'ListBoxOption' and n.get('label') == 'New note' for n in c.nodes()), 'Accent-insensitive search missed Café'


def empty_history(c, path):
    seed(c)
    c.execute('FocusNode', node=editor(c))
    c.execute('InjectKey', key='z', command=True)
    save(c)
    assert not records(path, 'block')[0]['trashed'], 'Empty text undo fell through into block creation history'


def stroke(c, path):
    seed(c, 'Sketch')
    block_menu(c, 'Draw')
    surface = node(c, 'Sketch block', 'Image')['bounds']
    x, y = surface['x'] + 30, surface['y'] + 30
    for phase, dx, dy in [('down', 0, 0), ('move', 100, 20), ('up', 120, 30)]:
        c.execute('InjectPointer', action=phase, x=x+dx, y=y+dy, kind='pen', pressure=.6)
    save(c)
    drawing = records(path, 'block')[0]['content']
    assert len(drawing['strokes']) == 1, drawing
    samples = drawing['strokes'][0]['samples']
    assert samples, drawing
    assert abs(samples[-1]['x'] - (150 * 560 / surface['width'])) < .1, samples[-1]
    assert abs(samples[-1]['y'] - (60 * 320 / surface['height'])) < .1, samples[-1]
    # Escape aborts provisional ink; it must not become a second persisted stroke.
    c.execute('FocusNode', node=node(c, 'Sketch block', 'Image')['id'])
    c.execute('InjectPointer', action='down', x=x, y=y, kind='pen', pressure=.5)
    c.execute('InjectPointer', action='move', x=x+50, y=y+50, kind='pen', pressure=.5)
    c.execute('InjectKey', key='Escape')
    c.execute('InjectPointer', action='up', x=x+50, y=y+50, kind='pen', pressure=.5)
    save(c)
    assert len(records(path, 'block')[0]['content']['strokes']) == 1
    menu(c, 'Edit', 'Undo draw stroke on this page')
    save(c)
    assert records(path, 'block')[0]['content']['strokes'][0]['trashed']
    menu(c, 'Edit', 'Redo draw stroke on this page')
    save(c)
    assert not records(path, 'block')[0]['content']['strokes'][0]['trashed']
    action(c, 'Erase', 'RadioButton')
    for phase in ['down', 'up']:
        c.execute('InjectPointer', action=phase, x=x+50, y=y+10)
    save(c)
    assert records(path, 'block')[0]['content']['strokes'][0]['trashed'], 'Eraser missed the segment between samples'


def backup(c, path):
    seed(c)
    type_text(c, 'Backup contents')
    menu(c, 'File', 'Back up now')
    files = [p for p in path.parent.glob('*.noton') if p != path]
    assert len(files) == 1, list(path.parent.iterdir())
    assert 'Backup contents' in records(files[0], 'block')[0]['content']['djot']


def table_edit(c, path):
    seed(c)
    c.execute('FocusNode', node=editor(c))
    table = lambda item: (action(c, 'Table'), action(c, item, 'MenuItem'))
    table('Insert 3 × 3 table')
    type_text(c, 'Cellule')
    table('Add row below')
    table('Add column after')
    save(c)
    payload = records(path, 'block')[0]['content']['djot']
    assert 'Cellule' in payload, payload
    def dimensions(text):
        rows = [line.strip()[1:-1].split('|') for line in text.splitlines() if line.strip().startswith('|') and '-' not in line]
        return len(rows), {len(row) for row in rows}
    assert dimensions(payload) == (4, {4}), payload
    table('Remove column')
    table('Remove row')
    save(c)
    payload = records(path, 'block')[0]['content']['djot']
    assert dimensions(payload) == (3, {3}), payload


def narrow(c, path):
    action(c, 'New folder')
    # The folder stage must lead to notes without exposing all three panes.
    action(c, 'New folder', 'ListBoxOption')
    action(c, 'New note')
    insert(c, 'Text block')
    type_text(c, 'Narrow page')
    action(c, 'Back')
    assert node(c, 'New note', 'Button')


def french(c, path):
    for label in ['Nouveau dossier', 'Nouvelle note']:
        action(c, label)
    action(c, 'Insérer')
    action(c, 'Bloc de texte', 'MenuItem')
    assert node(c, 'Rechercher dans le carnet', 'TextInput')
    assert node(c, 'Actions de la note')


def ime(c, path):
    seed(c)
    target = editor(c)
    c.execute('FocusNode', node=target)
    c.execute('TypeIme', node=target, preedit='préédition', commit='écriture 日本語')
    save(c)
    text = records(path, 'block')[0]['content']['djot']
    assert 'écriture 日本語' in text and 'préédition' not in text, text


def autosave(c, path):
    seed(c)
    type_text(c, 'Autosaved without pressing Save')
    deadline = time.monotonic()+6
    while time.monotonic() < deadline:
        c.nodes()  # Pump the headless host's real save coordinator.
        blocks = records(path, 'block')
        if blocks and 'Autosaved without pressing Save' in blocks[0]['content']['djot']:
            return
        time.sleep(.05)
    raise AssertionError('Autosave did not persist the committed edit')


def writer_lock(c, path):
    seed(c)
    save(c)
    # Creating Windows symlinks requires an OS privilege. Always test the
    # same-path lock there; the Linux/macOS run additionally tests canonical aliases.
    alias = path
    if sys.platform != 'win32':
        alias = path.parent/'alias.noton'
        alias.symlink_to(path)
    result = subprocess.run([str(pathlib.Path(sys.argv[1]).resolve()), '--mcp', '--library', str(alias)], input='', text=True, capture_output=True, timeout=10)
    assert result.returncode != 0 and 'already open' in result.stderr, result.stderr


def containment(c, path):
    seed(c)
    def inside(inner, outer):
        return inner['x'] >= outer['x'] and inner['y'] >= outer['y'] and inner['x']+inner['width'] <= outer['x']+outer['width']+1 and inner['y']+inner['height'] <= outer['y']+outer['height']+1
    card = node(c, 'Text block', 'Group')['bounds']
    edit = node(c, role='MultilineTextInput')['bounds']
    assert inside(edit, card), (edit, card)
    assert edit['width'] > card['width']*.8, 'Editor is stuck at intrinsic width'
    insert(c, 'Sketch')
    card = node(c, 'Sketch block', 'Group')['bounds']
    ink = node(c, 'Sketch block', 'Image')['bounds']
    assert inside(ink, card), (ink, card)


def main():
    output = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else '/tmp/noton-user-stories')
    output.mkdir(parents=True, exist_ok=True)
    results = []
    scenarios = [
        ('N01-N02-navigation-move-undo', navigation, ()),
        ('N03-list-reorder-undo', reorder, ()),
        ('E01-S04-multiline-projections', text_roundtrip, ()),
        ('E03-U01-formatting-history', formatting, ()),
        ('S01-U01-move-resize-history', movement, ()),
        ('U05-trash-restore', trash_restore, ()),
        ('Q01-accent-search', search, ()),
        ('U03-empty-text-history', empty_history, ()),
        ('I01-I02-I03-pen-cancel-history', stroke, ()),
        ('P04-X01-native-backup', backup, ()),
        ('T01-table-operations', table_edit, ()),
        ('A04-narrow-navigation', narrow, ('--window-size', '600x700')),
        ('L01-french-startup', french, ('--locale', 'fr-FR')),
        ('E02-IME-commit', ime, ()),
        ('P02-idle-autosave', autosave, ()),
        ('P06-writer-lock', writer_lock, ()),
        ('A04-card-containment', containment, ()),
    ]
    for name, scenario, args in scenarios:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory)/'story.noton'
            client = Client(path, args)
            try:
                scenario(client, path)
                client.close()
                result = {'scenario': name, 'status': 'passed'}
            except Exception as error:
                result = {'scenario': name, 'status': 'failed', 'error': str(error)}
                try:
                    (output/f'{name}-tree.json').write_text(json.dumps(client.nodes(), indent=2))
                except Exception:
                    pass
            finally:
                if client.p.poll() is None:
                    client.kill()
            results.append(result)
            print(f"{result['status'].upper()}: {name}: {result.get('error', '')}", flush=True)
    (output/'results.json').write_text(json.dumps(results, indent=2))
    return int(any(result['status'] != 'passed' for result in results))


if __name__ == '__main__':
    raise SystemExit(main())
