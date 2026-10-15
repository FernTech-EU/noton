#!/usr/bin/env python3
"""Native move/resize/Delete checks using an image_drop_fixture .noton file.

Usage: python tools/mcp_image_frames.py BINARY FIXTURE OUTPUT
Requires teksilo-automation-mcp on PATH and a desktop session.
"""
import json
import pathlib
import shutil
import sys
import tempfile
from mcp_ergonomics import Client
from mcp_stories import records, save


def main():
    out = pathlib.Path(sys.argv[3]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as directory:
        library = pathlib.Path(directory)/'images.noton'
        shutil.copyfile(sys.argv[2], library)
        c = Client(library, (), out)
        try:
            image = next(n for n in c.nodes() if n['role'] == 'Image' and n.get('label') == 'Image.png')
            b = image['bounds']
            c.call('inject_pointer', {'action': 'click', 'x': b['x']+b['width']/2, 'y': b['y']+b['height']/2})
            c.capture('selected-image')
            before = records(library, 'block')[0]['geometry']
            frame = next(n for n in c.nodes() if n['role'] == 'Group' and n.get('label') == 'Image.png')['bounds']
            x, y = frame['x']+80, frame['y']+12
            for action, dx, dy in [('down', 0, 0), ('move', 20, 15), ('move', 60, 40), ('up', 60, 40)]:
                c.call('inject_pointer', {'action': action, 'x': x+dx, 'y': y+dy})
            save(c)
            moved = records(library, 'block')[0]['geometry']
            assert moved['x'] > before['x'] and moved['y'] > before['y'], (before, moved)
            handle = next(n for n in c.nodes() if n.get('label') == 'Resize bottom trailing')['bounds']
            x, y = handle['x']+handle['width']/2, handle['y']+handle['height']/2
            for action, dx, dy in [('down', 0, 0), ('move', 15, 10), ('move', 50, 30), ('up', 50, 30)]:
                c.call('inject_pointer', {'action': action, 'x': x+dx, 'y': y+dy})
            save(c)
            resized = records(library, 'block')[0]['geometry']
            assert resized['width'] > moved['width'] and resized['height'] > moved['height'], (moved, resized)
            c.capture('moved-resized-image')
            c.execute('InjectKey', key='Delete')
            save(c)
            assert records(library, 'block')[0]['trashed'], 'Delete did not remove the selected image frame'
            c.execute('InjectKey', key='z', command=True)
            save(c)
            assert not records(library, 'block')[0]['trashed'], 'Undo did not restore the image'
            c.capture('restored-image')
            c.insert('Sketch')
            c.execute('InjectKey', key='Delete')
            save(c)
            sketches = [b for b in records(library, 'block') if b['content']['kind'] == 'Drawing']
            assert sketches and all(b['trashed'] for b in sketches), 'Delete after insertion did not remove the sketch'
            c.insert('Text block')
            editor = next(n for n in c.nodes() if n['role'] == 'MultilineTextInput')
            c.execute('FocusNode', node=editor['id'])
            c.execute('TypeText', node=editor['id'], text='ABC')
            c.execute('InjectKey', key='Home')
            c.execute('InjectKey', key='Delete')
            save(c)
            texts = [b for b in records(library, 'block') if b['content']['kind'] == 'RichText']
            assert len(texts) == 1 and not texts[0]['trashed'] and 'BC' in texts[0]['content']['djot'], texts
            c.execute('InjectKey', key='Escape')
            c.execute('InjectKey', key='Delete')
            save(c)
            assert all(b['trashed'] for b in records(library, 'block') if b['content']['kind'] == 'RichText')
            (out/'result.json').write_text(json.dumps({'status': 'passed'}))
            print('PASS: native image selection, header drag, handle resize, Delete, and undo')
            c.close()
        finally:
            if c.p.poll() is None:
                c.kill()


if __name__ == '__main__':
    main()
