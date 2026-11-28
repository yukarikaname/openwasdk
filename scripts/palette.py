#!/usr/bin/env python3
"""Regenerate openxaml's palette from Microsoft's generic.xaml.

The theme colours WinUI ships are not in any XBF this project can read, but they are in
microsoft-ui-xaml's own MIT-licensed source. This flattens the Light and Default theme
dictionaries -- following StaticResource aliases -- into theme/palette.txt:

    theme<TAB>key<TAB>#AARRGGBB

Usage: scripts/palette.py [path to generic.xaml]
"""
import re
import sys
path = sys.argv[1] if len(sys.argv) > 1 else '/Volumes/Lenovo/Developer/microsoft-ui-xaml/dxaml/xcp/dxaml/themes/generic.xaml'
text = open(path, encoding='utf-8').read()
def section(name):
    m = re.search(r'<ResourceDictionary x:Key="%s">' % name, text)
    return text[m.end():] if m else ''
def until_close(body):
    depth, i = 1, 0
    while i < len(body):
        if body.startswith('<ResourceDictionary', i): depth += 1
        elif body.startswith('</ResourceDictionary>', i):
            depth -= 1
            if depth == 0: return body[:i]
        i += 1
    return body
colors, brushes, aliases = {}, {}, {}
for theme in ('Default', 'Light'):
    body = until_close(section(theme))
    for m in re.finditer(r'<Color x:Key="([^"]+)"[^>]*>\s*(#[0-9A-Fa-f]{6,8})\s*</Color>', body):
        colors[(theme, m.group(1))] = m.group(2)
    for m in re.finditer(r'<SolidColorBrush x:Key="([^"]+)"[^>]*Color="(#[0-9A-Fa-f]{6,8})"', body):
        brushes[(theme, m.group(1))] = m.group(2)
    for m in re.finditer(r'<SolidColorBrush x:Key="([^"]+)"[^>]*Color="\{(?:Static|Theme)Resource ([^}]+)\}"', body):
        brushes[(theme, m.group(1))] = 'ref:' + m.group(2)
    for m in re.finditer(r'<StaticResource x:Key="([^"]+)" ResourceKey="([^"]+)"', body):
        aliases[(theme, m.group(1))] = 'ref:' + m.group(2)
def resolve(theme, key, depth=0):
    if depth > 10: return None
    for table in (brushes, colors, aliases):
        v = table.get((theme, key))
        if v is None: continue
        return resolve(theme, v[4:], depth + 1) if isinstance(v, str) and v.startswith('ref:') else v
    return None
lines = ["# Theme colours from Microsoft's own generic.xaml (microsoft-ui-xaml, MIT).",
         '# theme<TAB>key<TAB>#AARRGGBB. Regenerate with scripts/palette.py; see theme/palette.rs.']
rows = 0
for theme, label in (('Light', 'light'), ('Default', 'dark')):
    keys = sorted({k for (t, k) in set(brushes) | set(aliases) | set(colors) if t == theme})
    for key in keys:
        value = resolve(theme, key)
        if value and value.startswith('#'):
            lines.append(f'{label}\t{key}\t{value}')
            rows += 1
out = '/Volumes/Lenovo/Developer/openwasdk/crates/openxaml/src/theme/palette.txt'
open(out, 'w').write('\n'.join(lines) + '\n')
print('resolved rows', rows)
