#!/usr/bin/env python3
"""Each README_<LANG>.md carries the English README's code blocks verbatim,
the same links, and the language list, so commands, paths and URLs cannot
drift between translations. Prose is reviewed by people.

Links are absolute because crates.io renders README.md too; a link into this
repository's main branch must name a file that exists."""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
english = (root / 'README.md').read_text()
code = re.findall(r'```[a-z]*\n.*?\n```', english, flags=re.S)
repo_file = re.compile(r'https://(?:github\.com/rush-db/rushdb/(?:blob|tree)/main|raw\.githubusercontent\.com/rush-db/rushdb/main)/([^)#\s"]+)')
language = re.compile(r'https://github\.com/rush-db/rushdb/blob/main/(README(?:_[A-Z]{2})?\.md)')

def all_links(text):
    return [a or b for a, b in re.findall(r'\]\(([^)\s]+)\)|src="([^"]+)"', text)]

def links(text):
    """Every link target except the language list (each file leaves itself out)."""
    return sorted({link for link in all_links(text) if not language.fullmatch(link)})

def language_list(text):
    return sorted(m.group(1) for link in all_links(text) if (m := language.fullmatch(link)))

translations = sorted(root.glob('README_*.md'))
names = [path.name for path in translations]

failures = []
if language_list(english) != names:
    failures.append(f'README.md language list {language_list(english)} does not match {names}')
for path in translations:
    text = path.read_text()
    if re.findall(r'```[a-z]*\n.*?\n```', text, flags=re.S) != code:
        failures.append(f'{path.name}: code blocks differ from README.md')
    if links(text) != links(english):
        failures.append(f'{path.name}: links differ from README.md: '
                        f'{set(links(text)) ^ set(links(english))}')
    if language_list(text) != sorted({'README.md', *names} - {path.name}):
        failures.append(f'{path.name}: language list does not name README.md and every other translation')
for path in [root / 'README.md', *translations]:
    for link in all_links(path.read_text()):
        if link.startswith(('http:', 'https:', 'mailto:', '#')):
            match = repo_file.match(link)
            if match is None:
                continue
            link = match.group(1)
        if not (root / link).exists():
            failures.append(f'{path.name}: broken link {link}')
if failures:
    print('\n'.join(failures), file=sys.stderr)
    sys.exit(1)
print(f'README translations: {len(translations)} in step with README.md')
