extern crate rand;
use rand::{distributions::Uniform, prelude::*, seq::IteratorRandom};

use crate::cmd::sample::DefaultRng;

use super::*;

#[test]
fn simple() {
    let t = Tester::new();
    t.temp_file("sample", Some(&FASTA), |path, _| {
        // very simple tests
        t.cmp(&["sample", "-n", "4"], FileInput(path), &FASTA)
            .fails(
                &["sample", "-p", "2"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            )
            .fails(
                &["sample", "-p", "1"],
                FileInput(path),
                "Fractions should be between 0 and 1",
            );
    });
}

#[test]
fn large() {
    // RNGs and seeds
    // test with integer seed
    let seed1 = 602993;
    // string seed
    let seed2 = "ABCDEFGHIJKLMNOP";
    let mut seed2_array = [0; 32];
    (&mut seed2_array[..]).write_all(seed2.as_bytes()).unwrap();
    let rngs: Vec<(String, Box<dyn Fn() -> DefaultRng>)> = vec![
        (
            format!("{}", seed1),
            Box::new(|| DefaultRng::seed_from_u64(seed1)),
        ),
        (
            seed2.to_string(),
            Box::new(|| DefaultRng::from_seed(seed2_array)),
        ),
    ];

    // input
    let n_records = 1000;
    let seqs: Vec<_> = (0..n_records)
        .map(|i| format!(">{}\nSEQ\n", i))
        .collect();
    let fasta = seqs.join("");

    let t = Tester::new();
    t.temp_file("sample", Some(&fasta), |path, _| {
        for (seed, get_rng) in &rngs {
            // test fixed number (-n)
            for n in [1, 10, 100, 500, 998, 1000] {
                // also test different memory limits to ensure that switching
                // from sampling whole records to indices only works
                for rec_limit in [1, 5, 10, 100, 200, 500, 800, 1000, 10000] {
                    for two_pass in [false, true] {
                        // expected output:
                        // we use reservoir sampling implemented in the rand crate,
                        // which is a way of validating our own reimplementation.
                        let mut rng = get_rng();
                        let mut indices = (0..n_records).choose_multiple(&mut rng, n);
                        indices.sort(); // results always in input order
                        let expected = indices.into_iter().map(|i| seqs[i].clone()).join("");
                        // run sample command
                        let mem_limit = rec_limit * n * 12;
                        let n = format!("{}", n);
                        let mem = format!("{}", mem_limit);
                        let mut args = vec!["sample", "-n", &n, "-s", seed, "--max-mem", &mem];
                        if two_pass {
                            args.push("-2");
                        }
                        t.cmp(&args, FileInput(path), &expected);
                    }
                }
            }

            // test probability sampling (-p)
            let distr = Uniform::new(0f32, 1.);
            for &p in &[0., 0.1, 0.3, 0.5, 0.7, 0.95] {
                let mut rng = get_rng();
                let expected = seqs
                    .iter().filter(|&_| distr.sample(&mut rng) < p).cloned()
                    .join("");

                t.cmp(
                    &["sample", "-p", &format!("{}", p), "-s", seed],
                    FileInput(path),
                    &expected,
                );
            }
        }
    });
}
