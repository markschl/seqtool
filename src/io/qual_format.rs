/// See also https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2847217/pdf/gkp1137.pdf

use std::cmp::{min, max};
use std::fmt::Debug;


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QualFormat {
    /// Sanger, Illumina 1.8+, SRA: Offset 33 (0 to 93; theoretically)
    Sanger,
    /// Illumina 1.3 - 1.7: Offset 64 (0 to 62)
    Illumina,
    /// Solexa: Offset 64 (-5 to 62)
    Solexa,
    /// Direct phred scores (as from .qual file)
    Phred,
}

impl QualFormat {
    pub fn get_converter<'a>(&self) -> QualConverter {
        QualConverter::new(*self)
    }
}

use self::QualFormat::*;


#[derive(Debug)]
pub struct QualConverter {
    fmt: QualFormat,
}

impl QualConverter {
    pub fn new(fmt: QualFormat) -> QualConverter {
        QualConverter { fmt: fmt }
    }

    pub fn convert_quals(&self, qual: &[u8], out: &mut Vec<u8>, format: QualFormat) -> Result<(), String> {
        for &q in qual {
            out.push(self.convert(q, format)?);
        }
        Ok(())
    }

    pub fn prob_sum(&self, qual: &[u8]) -> Result<f64, String> {
        let mut prob = 0.;
        for &q in qual {
            prob += self.get_prob(q)?;
        }
        Ok(prob)
    }

    pub fn convert(&self, q: u8, to: QualFormat) -> Result<u8, String> {

        Ok(match self.fmt {
            Sanger => {
                let q = validate_sanger(q)?;
                match to {
                    // TODO: should there be a warning about truncated qualities?
                    Illumina => min(q, 95) + 31,
                    Phred  => q - 33,
                    Solexa   => qual_to_solexa(q - 33),
                    Sanger => q
                }
            }
            Phred => {
                // qualities are silently truncated, which should be
                // ok since such high Phred qualities will not occur
                // in reality
                match to {
                    // TODO: should there be a warning about truncated qualities?
                    Sanger => min(q, 92) + 33,
                    Illumina  => min(q, 62) + 64,
                    Solexa => qual_to_solexa(min(q, 62)),
                    Phred => q,
                }
            }
            Illumina => {
                let q = validate_illumina(q)?;
                match to {
                    Sanger => q - 31,
                    Phred  => q - 64,
                    Solexa => qual_to_solexa(q - 64),
                    Illumina => q,
                }
            }
            Solexa => {
                let q = validate_solexa(q)?;
                let offset = match to {
                    Sanger => 33,
                    Illumina => 64,
                    Phred => 0,
                    Solexa => return Ok(q),
                };
                let q = solexa_to_qual(validate_solexa(q)?);
                q + offset
            }
        })
    }

    // pub fn get_phred(&self, q: u8) -> Result<u8, String> {
    //
    //     Ok(match self.fmt {
    //         Sanger => validate_sanger(q)? - 33,
    //         Phred => q,
    //         Illumina => validate_illumina(q)? - 64,
    //         Solexa => solexa_to_qual(q),
    //     })
    // }

    pub fn get_prob(&self, q: u8) -> Result<f64, String> {

        Ok(match self.fmt {
            Sanger => qual_to_prob(validate_sanger(q)? - 33),
            Phred => qual_to_prob(q),
            Illumina => qual_to_prob(validate_illumina(q)? - 64),
            Solexa => solexa_to_prob(q),
        })
    }
}


macro_rules! validate_impl {
    ($name:ident, $min_ascii:expr, $min_char:expr, $fmt:expr) => {
        #[inline(always)]
        fn $name(q: u8) -> Result<u8, String> {
            if q > 126 {
                return Err(high_qual_err(q))
            } else if q < $min_ascii {
                return Err(low_qual_err(q, $min_ascii, $fmt))
            }
            Ok(q)
        }
    }
}

#[inline(never)]
fn high_qual_err(q: u8) -> String {
    format!(concat!(
        "Invalid quality score encountered ({}). ASCII codes > 126 ",
        "(= PHRED scores > 93) are not valid. "
    ), q)
}

#[inline(never)]
fn low_qual_err(q: u8, min_ascii: u8, fmt: &str) -> String {
    let fmt_guess = guess_format(q)
        .unwrap_or_else(|| "".to_string());
    format!(concat!(
        "Invalid quality score encountered ({}). In the {} FASTQ format, ",
        "the values should be in the ASCII range {}-126 ('{}' to '~').{}"
    ), q, fmt, min_ascii, min_ascii as char, fmt_guess
    )
}

validate_impl!(validate_sanger,   33, '!', "Sanger/Illumina 1.8+");
validate_impl!(validate_illumina, 64, '@', "Illumina 1.3+");
validate_impl!(validate_solexa,   59, ';', "Solexa");


#[inline]
fn guess_format(q: u8) -> Option<String> {
    let s =
        match q {
            0  ... 32 => None,
            33 ... 58 => Some(("Sanger/Illumina 1.8+", "'--fmt fq'")),
            59 ... 63 => Some(("Sanger/Illumina 1.8+ or eventually Solexa", "'--fmt fq' or '--fmt fq-solexa'")),
            _ => None
        };
    s.map(|(name, usage)| format!(
        " It seems that the file is in the {} format. If so, use the option {}.",
        name, usage
    ))
}

#[inline]
fn solexa_to_qual(q: u8) -> u8 {
    (10. * (10f64.powf((q as f64 - 64.) / 10.) + 1.).log10())
        .round() as u8
}

#[inline]
fn qual_to_solexa(q: u8) -> u8 {
    let s = ((10. * (10f64.powf(q as f64 / 10.) - 1.).log10()).round() + 64.) as u8;
    min(126, max(59, s))
}

#[inline]
fn solexa_to_prob(q: u8) -> f64 {
    1f64 / (10f64.powf((q as f64 - 64.) / 10.) + 1.)
}

#[inline]
pub fn qual_to_prob(q: u8) -> f64 {
    10f64.powf(-(q as f64) / 10.)
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::QualFormat::*;

    #[test]
    fn solexa_qual() {

        // according to https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2847217/
        let qual        = [ 1u8,  1,  1,  2,  2,  3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 10, 11];
        let solexa      = [-5i8, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8,  9, 10, 11];
        let solexa_back = [-5i8, -5, -5, -2, -2,  0, 0, 2, 2, 3, 3, 5, 6, 7, 8, 10, 10, 11];

        assert_eq!(qual_to_solexa(0), 59);

        for ((&q, &s), &sb) in (&qual[..]).into_iter()
                                   .zip(&solexa[..])
                                   .zip(&solexa_back[..]) {
            let s = (s + 64) as u8;
            let sb = (sb + 64) as u8;
            assert_eq!(solexa_to_qual(s), q);
            assert_eq!(qual_to_solexa(q), sb);
            assert_eq!(solexa_to_qual(qual_to_solexa(q)), q);
            assert_eq!(qual_to_solexa(solexa_to_qual(sb)), sb);
        }

        // these should be equal
        for q in 12..62 {
            assert_eq!(qual_to_solexa(q), q + 64);
        }
    }

    // e.g. http://www.somewhereville.com/2011/12/16/sanger-and-illumina-1-3-and-solexa-phred-score-q-ascii-glyph-base-error-conversion-tables/
    #[test]
    fn probs() {
        let mapping = [
            (0u8, 1f64),
            ( 1, 0.7943282347),
            (10, 0.1),
            (40, 0.0001000000),
            (93, 0.0000000005),
        ];
        let f = 10f64.powi(10);
        for &(q, p) in &mapping[..] {
            assert_eq!((qual_to_prob(q) * f).round() / f, p);
        }
    }

    #[test]
    fn probs_solexa() {
        let mapping = [
            (-5i8, 0.7597469f64),
            ( 0, 0.5),
            ( 1, 0.4426884),
            (10, 0.0909091),
            (40, 0.0001000),
            (62, 0.0000006),
        ];
        let f = 10f64.powi(7);
        for &(q, p) in &mapping[..] {
            assert_eq!((solexa_to_prob((q + 64) as u8) * f).round() / f, p);
        }
    }

    #[test]
    fn convert_sanger() {

        let q = QualConverter::new(Sanger);

        assert_eq!(q.convert(33, Sanger), Ok(33));
        assert_eq!(q.convert(43, Sanger), Ok(43));

        assert_eq!(q.convert(33, Illumina), Ok(64));
        assert_eq!(q.convert(43, Illumina), Ok(74));

        assert_eq!(q.convert(33, Phred), Ok(0));
        assert_eq!(q.convert(43, Phred), Ok(10));

        assert_eq!(q.convert(33, Solexa), Ok(59));
        assert_eq!(q.convert(43, Solexa), Ok(74));

        // Out of range
        assert!(q.convert(32, Sanger).is_err());
        assert!(q.convert(127, Sanger).is_err());
    }

    #[test]
    fn convert_phred() {

        let q = QualConverter::new(Phred);

        assert_eq!(q.convert(0, Sanger), Ok(33));
        assert_eq!(q.convert(10, Sanger), Ok(43));

        assert_eq!(q.convert(0, Phred), Ok(0));
        assert_eq!(q.convert(10, Phred), Ok(10));

        assert_eq!(q.convert(0, Illumina), Ok(64));
        assert_eq!(q.convert(10, Illumina), Ok(74));

        assert_eq!(q.convert(0, Solexa), Ok(59));
        assert_eq!(q.convert(10, Solexa), Ok(74));

        // Out of range (not possible)
        assert!(q.convert(0, Phred).is_ok());
        assert!(q.convert(255, Phred).is_ok());
    }

    #[test]
    fn convert_illumina() {

        let q = QualConverter::new(Illumina);

        assert_eq!(q.convert(64, Sanger), Ok(33));
        assert_eq!(q.convert(74, Sanger), Ok(43));

        assert_eq!(q.convert(64, Phred), Ok(0));
        assert_eq!(q.convert(74, Phred), Ok(10));

        assert_eq!(q.convert(64, Illumina), Ok(64));
        assert_eq!(q.convert(74, Illumina), Ok(74));

        assert_eq!(q.convert(64, Solexa), Ok(59));
        assert_eq!(q.convert(74, Solexa), Ok(74));

        // Out of range
        assert!(q.convert(63, Illumina).is_err());
        assert!(q.convert(127, Illumina).is_err());
    }

    #[test]
    fn convert_solexa() {

        let q = QualConverter::new(Solexa);

        assert_eq!(q.convert(59, Sanger), Ok(34));
        assert_eq!(q.convert(74, Sanger), Ok(43));

        assert_eq!(q.convert(59, Phred), Ok(1));
        assert_eq!(q.convert(74, Phred), Ok(10));

        assert_eq!(q.convert(59, Illumina), Ok(65));
        assert_eq!(q.convert(74, Illumina), Ok(74));

        assert_eq!(q.convert(59, Solexa), Ok(59));
        assert_eq!(q.convert(74, Solexa), Ok(74));

        // Out of range
        assert!(q.convert(58, Solexa).is_err());
        assert!(q.convert(127, Solexa).is_err());
    }
}
