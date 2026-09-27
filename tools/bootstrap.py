#!/usr/bin/env python3
"""Resolve unpublished teksilo-scene from pinned sources, or use explicit local checkouts."""
import argparse,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parent.parent
p=argparse.ArgumentParser();p.add_argument('--teksilo',type=pathlib.Path);p.add_argument('--text-document',type=pathlib.Path);p.add_argument('--text-typeset',type=pathlib.Path);a=p.parse_args()
pins=json.loads((ROOT/'tools/dependencies.json').read_text())
paths={}
for name,spec in pins.items():
    explicit=getattr(a,name.replace('-','_'))
    path=explicit.resolve() if explicit else ROOT/'.dependencies'/name
    if not explicit:
        if not path.exists():
            path.mkdir(parents=True)
            subprocess.run(['git','init',str(path)],check=True)
            subprocess.run(['git','-C',str(path),'remote','add','origin',spec['repository']],check=True)
        head=subprocess.run(['git','-C',str(path),'rev-parse','HEAD'],capture_output=True,text=True)
        if head.returncode or head.stdout.strip()!=spec['revision']:
            if subprocess.check_output(['git','-C',str(path),'status','--porcelain'],text=True).strip():raise SystemExit(f'Refusing to overwrite changes in {path}')
            subprocess.run(['git','-C',str(path),'fetch','--depth=1','origin',spec['revision']],check=True)
            subprocess.run(['git','-C',str(path),'checkout','--detach',spec['revision']],check=True)
    if not (path/'Cargo.toml').is_file():raise SystemExit(f'No Cargo workspace at {path}')
    paths[name]=path
patches={'teksilo':paths['teksilo']/'crates/teksilo','teksilo-scene':paths['teksilo']/'crates/teksilo-scene','teksilo-text':paths['teksilo']/'crates/teksilo-text','text-document':paths['text-document']/'crates/public_api','text-typeset':paths['text-typeset']}
config=ROOT/'.cargo/config.toml';config.parent.mkdir(exist_ok=True)
content='[patch.crates-io]\n'+''.join(f'{name} = {{ path = {json.dumps(str(path))} }}\n' for name,path in patches.items())
if config.exists() and config.read_text()!=content:raise SystemExit(f'{config} differs; preserve it before choosing other overrides')
config.write_text(content)
print('Dependency sources configured. Build with cargo build --locked.')
