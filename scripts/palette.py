#!/usr/bin/env python3
"""Regenerate openxaml's palette from Microsoft's own theme XAML.

WinUI's colours are not in any XBF this project can read, but they are in
microsoft-ui-xaml's MIT-licensed theme files:

  dxaml/xcp/dxaml/themes/generic.xaml                       WinUI 2 aliases and colours
  controls/dev/CommonStyles/Common_themeresources_any.xaml   WinUI 3 tokens (Card*, Layer*)

Each file keeps a Default (dark), Light and HighContrast dictionary; this flattens the
Dark and Light ones -- following StaticResource/ThemeResource aliases -- into
theme/palette.txt as:

    theme<TAB>key<TAB>#AARRGGBB

Usage: scripts/palette.py [file ...]
"""
import os
import re
import subprocess
import sys

# The reference checkout: generic.xaml holds the WinUI 2 aliases, every control's
# *_themeresources.xaml holds the WinUI 3 tokens that control uses. Both are MIT.
REPO = '/Volumes/Lenovo/Developer/microsoft-ui-xaml'
BASE = 'dxaml/xcp/dxaml/themes/generic.xaml'


def theme_files():
    """Every theme XAML the tokens can live in, read from the repository's object store.

    A sparse checkout usually holds only a few of these files; the object store has all
    of them, and `git show` reads one without materialising the tree.
    """
    listing = subprocess.check_output(
        ['git', '-C', REPO, 'ls-tree', '-r', '--name-only', 'HEAD'], text=True)
    names = [name for name in listing.splitlines()
             if name.endswith('.xaml') and '_themeresources' in name]
    return [BASE] + sorted(names)


def read(rel):
    path = os.path.join(REPO, rel)
    if os.path.exists(path):
        return open(path, encoding='utf-8').read()
    return subprocess.check_output(['git', '-C', REPO, 'show', 'HEAD:' + rel],
                                   text=True, errors='replace')
OUT = '/Volumes/Lenovo/Developer/openwasdk/crates/openxaml/src/theme/palette.txt'

colors, brushes, aliases = {}, {}, {}


def section(text, name):
    m = re.search(r'<ResourceDictionary x:Key="%s">' % name, text)
    return text[m.end():] if m else ''


def until_close(body):
    depth, i = 1, 0
    while i < len(body):
        if body.startswith('<ResourceDictionary', i):
            depth += 1
        elif body.startswith('</ResourceDictionary>', i):
            depth -= 1
            if depth == 0:
                return body[:i]
        i += 1
    return body


def harvest(text):
    for theme in ('Default', 'Light'):
        body = until_close(section(text, theme))
        for m in re.finditer(r'<Color x:Key="([^"]+)"[^>]*>\s*(#[0-9A-Fa-f]{6,8})\s*</Color>', body):
            colors[(theme, m.group(1))] = m.group(2)
        for m in re.finditer(r'<SolidColorBrush x:Key="([^"]+)"[^>]*Color="(#[0-9A-Fa-f]{6,8})"', body):
            brushes[(theme, m.group(1))] = m.group(2)
        for m in re.finditer(r'<SolidColorBrush x:Key="([^"]+)"[^>]*Color="\{(?:Static|Theme)Resource ([^}]+)\}"', body):
            brushes[(theme, m.group(1))] = 'ref:' + m.group(2)
        for m in re.finditer(r'<StaticResource x:Key="([^"]+)" ResourceKey="([^"]+)"', body):
            aliases[(theme, m.group(1))] = 'ref:' + m.group(2)


def resolve(theme, key, depth=0):
    if depth > 10:
        return None
    for table in (brushes, colors, aliases):
        value = table.get((theme, key))
        if value is None:
            continue
        return resolve(theme, value[4:], depth + 1) if value.startswith('ref:') else value
    return None


if sys.argv[1:]:
    for path in sys.argv[1:]:
        harvest(open(path, encoding='utf-8').read())
else:
    for path in theme_files():
        harvest(read(path))

lines = ["# Theme colours from Microsoft's own theme XAML (microsoft-ui-xaml, MIT).",
         '# theme<TAB>key<TAB>#AARRGGBB. Regenerate with scripts/palette.py.']
rows = 0
for theme, label in (('Light', 'light'), ('Default', 'dark')):
    keys = sorted({k for (t, k) in set(brushes) | set(aliases) | set(colors) if t == theme})
    for key in keys:
        value = resolve(theme, key)
        if value and value.startswith('#'):
            lines.append(f'{label}\t{key}\t{value}')
            rows += 1
open(OUT, 'w').write('\n'.join(lines) + '\n')
print('resolved rows', rows)
