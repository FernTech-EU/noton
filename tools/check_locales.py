#!/usr/bin/env python3
"""Check application Fluent key parity and reject duplicate definitions."""
import pathlib,re
root=pathlib.Path(__file__).resolve().parent.parent/'crates/teksilo_ui/locales'
sets=[]
for locale in ['en-US','fr-FR']:
    keys=re.findall(r'^([a-z][a-z0-9-]*)\s*=',(root/f'{locale}.ftl').read_text(),re.M)
    if len(keys)!=len(set(keys)):raise SystemExit(f'Duplicate Fluent key in {locale}')
    sets.append(set(keys))
if sets[0]!=sets[1]:raise SystemExit(f'Locale mismatch: {sets[0]^sets[1]}')
print(f'Both locales contain {len(sets[0])} matching keys.')
