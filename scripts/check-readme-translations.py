#!/usr/bin/env python3
"""Each README_<LANG>.md carries the English README's code blocks verbatim,
the same links to repository files, and the language bar, so commands and
paths cannot drift between translations. Prose is reviewed by people."""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
english = (root / 'README.md').read_text()
code = re.findall(r'```[a-z]*\n.*?\n```', english, flags=re.S)

def local_links(text):
    """Links to repository files, without the language bar (each file leaves itself out)."""
    links = re.findall(r'\]\(((?!https?:)[^)#]+)', text)
    return sorted({link for link in links if not re.fullmatch(r'README(_[A-Z]{2})?\.md', link)})

translations = sorted(root.glob('README_*.md'))
languages = re.findall(r'\[[^\]]+\]\((README(?:_[A-Z]{2})?\.md)\)', english)

failures = []
if sorted(languages) != sorted(path.name for path in translations):
    failures.append(f'README.md language bar {languages} does not match {[p.name for p in translations]}')
for path in translations:
    text = path.read_text()
    if re.findall(r'```[a-z]*\n.*?\n```', text, flags=re.S) != code:
        failures.append(f'{path.name}: code blocks differ from README.md')
    if local_links(text) != local_links(english):
        failures.append(f'{path.name}: repository links differ from README.md: '
                        f'{set(local_links(text)) ^ set(local_links(english))}')
for path in [root / 'README.md', *translations]:
    for link in local_links(path.read_text()):
        if not (root / link).exists():
            failures.append(f'{path.name}: broken link {link}')
if failures:
    print('\n'.join(failures), file=sys.stderr)
    sys.exit(1)
print(f'README translations: {len(translations)} in step with README.md')
