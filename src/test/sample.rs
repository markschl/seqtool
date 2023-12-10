extern crate rand;
use rand::{distributions::Uniform, prelude::*};

use super::*;

#[test]
fn sample() {
    let t = Tester::new();

    t.temp_file("sample", Some(*FASTA), |path, _| {
        t.cmp(&["sample", "-n", "4"], FileInput(path), &FASTA)
            .cmp(&["sample", "-n", "0"], FileInput(path), "")
            .fails(
                &["sample", "-f", "2"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            )
            .fails(
                &["sample", "-f", "1.2"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            );

        // integer seed
        let mut seed1 = [0; 32];
        seed1[0] = 9;
        let seed2_vec: Vec<_> = (65..97).collect();
        // string seed
        let mut seed2 = [0; 32];
        (&mut seed2[..]).write_all(&seed2_vec).unwrap();

        let seeds = vec![
            (seed1, "9"),
            (seed2, std::str::from_utf8(&seed2[..]).unwrap()),
        ];

        for (seed, seed_str) in seeds {
            for &p in &[0., 0.5, 1.] {
                let mut rng = StdRng::from_seed(seed);
                let distr = Uniform::new_inclusive(0f32, 1.);
                let expected = SEQS
                    .iter()
                    .cloned()
                    .filter(|_| distr.sample(&mut rng) < p)
                    .collect::<Vec<_>>()
                    .concat();

                t.cmp(
                    &["sample", "-f", &format!("{}", p), "-s", seed_str],
                    FileInput(path),
                    &expected,
                );
            }
        }
    });
}
