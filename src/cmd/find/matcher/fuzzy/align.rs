use std::cmp::min;
use bio::alignment::pairwise;
use bio::alignment::AlignmentOperation;

use super::super::Match;

pub struct Aligner<A>
where
    A: Fn(u8, u8) -> i32,
{
    aligner: pairwise::Aligner<A>,
    score_fn: A,
    pattern: Vec<u8>,
}

impl<A> Aligner<A>
where
    A: Fn(u8, u8) -> i32 + Copy,
{
    pub fn new(pattern: &[u8], score_fn: A) -> Aligner<A> {
        let mut s = pairwise::Scoring::new(-1, -1, score_fn);
        s.xclip_prefix = pairwise::MIN_SCORE;
        s.xclip_suffix = pairwise::MIN_SCORE;
        s.yclip_prefix = 0;
        s.yclip_suffix = pairwise::MIN_SCORE;
        let aligner = pairwise::Aligner::with_scoring(s);
        Aligner {
            pattern: pattern.to_owned(),
            score_fn: score_fn,
            aligner: aligner,
        }
    }

    pub fn align(&mut self, text: &[u8], m: &mut Match)
    where
        A: Fn(u8, u8) -> i32,
    {
        let l = self.pattern.len();
        // convert to range end instead of index
        m.end += 1;

        if m.dist == 0 {
            m.start = m.end - min(m.end, l);
        } else {
            let check_start = m.end - min(m.end, l + m.dist as usize + 1);
            let sub = &text[check_start..m.end];
            let aln = self.aligner.custom(&self.pattern, sub);
            let (subst, ins, del) = get_diff(&aln.operations, &self.pattern, sub, |a, b| {
                (self.score_fn)(a, b) > 0
            });
            m.start = check_start + aln.ystart;
            m.subst = subst as u16;
            m.ins = ins as u16;
            m.del = del as u16;
        }
    }
}

// Returns the distance between x & y given information from an alignment.
// However, a custom function decides if characters in x & y match or not
// even if they are not equal.
fn get_diff<F>(
    operations: &[AlignmentOperation],
    x: &[u8],
    y: &[u8],
    func: F,
) -> (usize, usize, usize)
where
    F: Fn(u8, u8) -> bool,
{
    let mut subst = 0;
    let mut ins = 0;
    let mut del = 0;

    let mut ix = 0;
    let mut iy = 0;
    for op in operations {
        match *op {
            AlignmentOperation::Match => {
                ix += 1;
                iy += 1;
            }
            AlignmentOperation::Subst => {
                if !func(x[ix], y[iy]) {
                    subst += 1;
                }
                ix += 1;
                iy += 1;
            }
            AlignmentOperation::Del => {
                iy += 1;
                del += 1;
            }
            AlignmentOperation::Ins => {
                ix += 1;
                ins += 1;
            }
            AlignmentOperation::Xclip(n) => {
                ix += n;
                //diff += n;
            }
            AlignmentOperation::Yclip(n) => {
                iy += n;
            }
        }
    }
    (subst, ins, del)
}
