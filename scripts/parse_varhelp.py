#!/usr/bin/env python3

import sys
import re

prev_line = ''
in_table = False
for line in sys.stdin:
    if line.startswith('---'):
        prev_line = '### {}\n'.format(re.sub('(.+?Usage: )(.*)', '\\1`\\2`', prev_line))
        line = next(sys.stdin)
        if line[0].islower():
            prev_line = prev_line + '\n| variable | description |\n| - | - '
            in_table = True

    if in_table:
        if line.startswith('Example') or len(line) == 0:
            in_table = False
        else:
            #line = line[1:]
            if line[0] != ' ':
                prev_line += '|\n'
                s = line.strip().split(' ', 1)
                if len(s) == 2:
                    name, desc = s
                    line = '| {} | {} '.format(name, desc)
            else:
                prev_line += ' ' + line.strip()
                continue

    if line.startswith('Example'):
        line = '#### {}\n\n'.format(line.strip())

    elif line.startswith('> '):
        line = '\n```bash\n{}```\n\n'.format(line[2:])

    print(prev_line, end='')
    prev_line = line

print(prev_line, end='')
