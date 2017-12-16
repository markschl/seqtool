#!/usr/bin/env python3

bases = ['A', 'C', 'G', 'T', 'U']

mapping = [
    ('M', 'AC'),
    ('R', 'AG'),
    ('W', 'ATU'),
    ('S', 'CG'),
    ('Y', 'CTU'),
    ('K', 'GTU'),
    ('V', 'ACG'),
    ('H', 'ACTU'),
    ('D', 'AGTU'),
    ('B', 'CGTU'),
    ('N', 'ACGTU'),
    ('?', '?')
]

mapping = [(b, b) for b in bases] + mapping

indices = {b: i for i, (b, _) in enumerate(mapping)}

idx_map = [16] * 256

for b, _ in mapping:
    idx_map[ord(b)] = indices[b]

print('IDX: static [u8; 256] = [')
for i in range(0, 256, 16):
    print('    ' + ''.join('{:<2}, '.format(b) for b in idx_map[i : i + 16]))
print('];')

print('MATRIX: static [i8; 289] = [')
print('  // ' + ''.join('{:<4}'.format(i) for i in range(17)))
print('  // ' + '   '.join(b for b, _ in mapping))
for code1, bases1 in mapping:
    print('    ', end='')
    for code2, bases2 in mapping:
        if (all(b in bases2 for b in bases1) or code1 == 'T' and code2 == 'U' or code2 == 'T' and code1 == 'U') \
           and code1 != '?':
            print(' 1', end='')
        else:
            print('-1', end='')
        print(', ', end='')
    print('  // {}'.format(code1))
print('];')
