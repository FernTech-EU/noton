#!/usr/bin/env python3
"""Linux native idle/autosave regression; drives only the process it launches.

Usage: python tools/check_native_scheduling.py target/debug/noton /tmp/fresh-output
Requires a desktop session and a matching teksilo-automation-mcp on PATH.
Focus stays off text inputs during idle measurement to exclude intentional caret blink.
No MCP calls occur while waiting for autosave: SQLite observations cannot wake the UI.
"""
import json, os, pathlib, re, sqlite3, subprocess, sys, time
from mcp_live_client import LiveClient
assert sys.platform.startswith('linux'), 'Idle measurement uses Linux /proc'
binary=str(pathlib.Path(sys.argv[1]).resolve())
out=pathlib.Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=False)
env=dict(os.environ,XDG_CONFIG_HOME=str(out/'config'),XDG_DATA_HOME=str(out/'data'),TEKSILO_IDLE_TRACE='1')
path=out/'native.noton'
log=(out/'app.log').open('w')
p=subprocess.Popen([binary,'--library',str(path),'--locale','en-US'],env=env,stdout=log,stderr=log)
c=None
try:
    descriptor=pathlib.Path(os.environ['XDG_RUNTIME_DIR'])/'teksilo-automation'/f'{p.pid}.json'
    limit=time.monotonic()+30
    while not descriptor.exists():
        assert p.poll() is None, 'App exited: '+(out/'app.log').read_text()
        assert time.monotonic()<limit,'Bridge timeout'
        time.sleep(.1)
    c=LiveClient(p.pid,out)
    c.call('list_windows')
    c.call('settle')
    button=c.find('New folder')
    c.call('focus_node',{'node':button['id']})
    c.call('settle')
    # Wake once after each quiet interval to make the framework report its counters.
    time.sleep(1.2)
    c.call('snapshot_tree')
    time.sleep(1.2)
    c.call('snapshot_tree')
    time.sleep(.4)  # Let the observation's own redraw/deferred work finish.
    def main_switches():
        status=pathlib.Path(f'/proc/{p.pid}/task/{p.pid}/status').read_text()
        return int(next(line.split(':')[1] for line in status.splitlines() if line.startswith('voluntary_ctxt_switches:')))
    idle_start=len((out/'app.log').read_text())
    switches_before=main_switches()
    time.sleep(2.5)
    switches_after=main_switches()
    # OS/Wayland notifications can wake the thread without drawing. The root
    # regression is recurring rendering; do not mistake OS wakes for app polling.
    # A trace report may include the preceding snapshot's final redraw because
    # counters reset at report time, not at our measurement boundary.
    quiet_log=(out/'app.log').read_text()[idle_start:]
    frames=sum(map(int,re.findall(r'rendered_frames=(\d+)',quiet_log)))
    measurement={'seconds':2.5,'main_thread_voluntary_switches_before':switches_before,
        'main_thread_voluntary_switches_after':switches_after,'reported_frames':frames}
    (out/'idle-measurement.json').write_text(json.dumps(measurement,indent=2))
    assert frames<=1, ('recurring redraws during quiet interval',measurement,quiet_log)
    c.call('snapshot_tree')
    idle_log=(out/'app.log').read_text()[idle_start:]
    (out/'idle-window.log').write_text(idle_log)
    c.click('New folder');c.click('New note');c.click('Insert');c.click('Text block','MenuItem')
    editor=next(n for n in c.nodes() if n['role']=='MultilineTextInput')
    c.call('focus_node',{'node':editor['id']})
    c.call('type_text',{'node':editor['id'],'text':'Native autonomous save proof'})
    # No more UI/MCP calls: disk observation cannot wake the application's event loop.
    limit=time.monotonic()+6
    persisted=False
    while time.monotonic()<limit:
        with sqlite3.connect(f'file:{path}?mode=ro',uri=True) as db:
            persisted=any('Native autonomous save proof' in row[0] for row in db.execute('select payload from records'))
        if persisted:break
        time.sleep(.1)
    assert persisted,'Last edit did not autosave without another UI event'
    # Let the worker's own notification update the UI before querying it.
    time.sleep(.4)
    nodes=c.call('snapshot_tree')['nodes']
    (out/'after-save-tree.json').write_text(json.dumps(nodes,indent=2))
    assert any(str(n.get('label') or '').startswith('Saved · ') or str(n.get('value') or '').startswith('Saved · ') for n in nodes), [(n.get('label'),n.get('value')) for n in nodes]
    c.call('inject_key',{'key':'q','command':True})
    p.wait(timeout=10)
    assert p.returncode==0
    print('PASS: native idle interval had no recurring redraws; last keystroke autosaved without UI/MCP input; completion updated Saved.')
finally:
    if c:c.close()
    if p.poll() is None:p.terminate();p.wait(timeout=10)
    log.close()
