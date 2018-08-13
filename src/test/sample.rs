extern crate rand;
use rand::{Rng, SeedableRng};

use super::*;

#[test]
fn sample() {
    let t = Tester::new();

    t.temp_file("sample", Some(*FASTA), |path, _| {
        t.cmp(&["sample", "-n", "4"], FileInput(path), &FASTA)
            .cmp(&["sample", "-n", "0"], FileInput(path), "\n")
            .fails(
                &["sample", "-f", "2"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            )
            .fails(
                &["sample", "-f", "-1"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            );

        let mut seed = [0; 32];
        seed[0] = 9;
        for &p in [0., 0.5, 1.].into_iter() {
            let mut rng = rand::StdRng::from_seed(seed);
            let expected = SEQS
                .iter()
                .cloned()
                .filter(|_| rng.gen::<f32>() < p)
                .collect::<Vec<_>>()
                .concat();
            t.cmp(
                &["sample", "-f", &format!("{}", p), "-s", "9"],
                FileInput(path),
                &expected,
            );
        }
    });
}
