#!/usr/bin/env python3

bases = ['A', 'C', 'G', 'T', 'U']

mapping = [
    ('M', 'AC'),
    ('R', 'AG'),
    ('W', 'AT'),
    ('S', 'CG'),
    ('Y', 'CT'),
    ('K', 'GT'),
    ('V', 'ACG'),
    ('H', 'ACT'),
    ('D', 'AGT'),
    ('B', 'CGT'),
    ('N', 'ACGT'),
]

for b, bases in mapping:
    other = [a for a, v in mapping if all(b in bases for b in v)]
    print("b'{}' => b\"{}\".to_vec(),".format(b, ''.join(list(bases) + other)))
