#![feature(test)]

extern crate test;
extern crate pattern_matching;
extern crate bio;

use test::Bencher;
use pattern_matching as pm;
use bio::alignment::*;

static TEXT: &'static [u8] = b"CGCCGCGGTGTCCGCGCGTGGGTCCTGAGGGAGCTCGTCGGTGTGGGGTTCGGGGCGGTTTGAGTGAGACGAGACGAGACGCGCCCCTCCCACGCGGGGAAGGGCGCCCGCCTGCTCTCGGTGAGCGCACGTCCCGTGCTCCCCTCTGGCGGGTGCGCGCGGGCCGTGTGAGCGATCGCGGTGGGTTCGGGCCGGTGTGACGCGTGCGCCGGCCGGCCGCCGAGGGGCTGCCGTTCTGCCTCCGACCGGTCGTGTGTGGGTTGACTTCGGAGGCGCTCTG";
static PATTERN: &'static [u8] = b"GAGACCGAGAGAGACGCGACC";
static K: usize = 6;


#[bench]
fn myers_bio(b: &mut Bencher) {
    use bio::pattern_matching::myers::Myers;
    let myers = Myers::new(PATTERN);
    b.iter(|| {
        for _ in myers.find_all_end(TEXT, K as u8) {}
    });
}

#[bench]
fn myers(b: &mut Bencher) {
    use pm::myers::Myers;
    let myers = Myers::new(PATTERN);
    b.iter(|| {
        let myers_m = myers.find_all_end(TEXT, K as u8);
        for _ in myers_m {}
    });
}

#[bench]
fn myers_pos(b: &mut Bencher) {
    use pm::myers::Myers;
    let mut myers = Myers::new(PATTERN);
    b.iter(|| {
        let myers_m = myers.find_all_pos(TEXT, K as u8);
        for _ in myers_m {}
    });
}

#[bench]
fn myers_path(b: &mut Bencher) {
    use pm::myers::Myers;
    let mut myers = Myers::new(PATTERN);
    let mut ops = vec![];
    b.iter(|| {
        let mut myers_m = myers.find_all_pos(TEXT, K as u8);
        while let Some(_) = myers_m.next_path(&mut ops) { }
    });
}
