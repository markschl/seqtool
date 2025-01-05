/// See also https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2847217/pdf/gkp1137.pdf
use std::cmp::min;
use std::fmt::Debug;
use std::str::FromStr;

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

impl FromStr for QualFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sanger" => Ok(QualFormat::Sanger),
            "illumina" => Ok(QualFormat::Illumina),
            "solexa" => Ok(QualFormat::Solexa),
            // minor inconsistency: phred should not be constructed from CLI
            _ => Err(format!("Unknown quality format: {}", s)),
        }
    }
}

use self::QualFormat::*;

/// Wrapper struct for Phred scores, also keeping the orignal scores in order
/// to allow the correct calculation of the total sequencing error estimate
pub struct PhredScores<'a> {
    orig_scores: &'a [u8],
    orig_format: QualFormat,
    phred: &'a [u8],
}

impl PhredScores<'_> {
    pub fn scores(&self) -> &[u8] {
        self.phred
    }

    pub fn total_error(&self) -> f64 {
        if self.orig_format != Solexa {
            total_error(self.phred)
        } else {
            total_error_solexa(self.orig_scores)
        }
    }
}

#[inline(never)]
fn high_qual_err(q: u8, offset: u8) -> String {
    format!(
        "Invalid quality score encountered ({} or {}). ASCII codes > 126 \
        (= PHRED scores > {}) are not valid. ",
        q,
        q as char,
        126 - offset
    )
}

#[inline(never)]
fn low_qual_err(q: u8, min_ascii: u8, fmt: &str) -> String {
    let fmt_guess = guess_format(q).unwrap_or_default();
    format!(
        "Invalid quality score encountered ({} or {}). \
        The {} FASTQ format requires values in the ASCII range {}-126.{}",
        q, q as char, fmt, min_ascii, fmt_guess
    )
}

#[derive(Debug)]
pub struct QualConverter {
    fmt: QualFormat,
    // Buffer for quality score conversion
    qual_buf: Vec<u8>,
}

impl QualConverter {
    pub fn new(fmt: QualFormat) -> QualConverter {
        QualConverter {
            fmt,
            qual_buf: Vec::new(),
        }
    }

    #[inline(always)]
    pub fn validate(&self, qual: &[u8]) -> Result<(), String> {
        macro_rules! validate {
            ($q:expr, $offset:expr, $min_ascii:expr, $fmt:expr) => {
                if $q > 126 {
                    return Err(high_qual_err($q, $offset));
                } else if $q < $min_ascii {
                    return Err(low_qual_err($q, $min_ascii, $fmt));
                }
            };
        }

        match self.fmt {
            Sanger => {
                for q in qual {
                    validate!(*q, 33, 33, "Sanger/Illumina 1.8+");
                }
            }
            // no restrictions imposed on Phred, even if values > 93 cannot
            // be represented by any FASTQ format
            // (has no practical importance anway)
            Phred => {}
            Illumina => {
                for q in qual {
                    validate!(*q, 64, 64, "Illumina 1.3-1.7");
                }
            }
            Solexa => {
                for q in qual {
                    validate!(*q, 64, 59, "Solexa");
                }
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub fn convert_to<'a>(
        &'a mut self,
        qual: &'a [u8],
        format: QualFormat,
    ) -> Result<&'a [u8], String> {
        if self.fmt == format {
            // no conversion needed
            return Ok(qual);
        }
        // make sure that there are no values out of range
        self.validate(qual)?;
        // Copy to internal buffer, so the conversion can be done in place
        // and can (should) profit from auto-vectorization
        self.qual_buf.clear();
        self.qual_buf.extend_from_slice(qual);
        for q in &mut self.qual_buf {
            convert(q, self.fmt, format);
        }
        Ok(&self.qual_buf)
    }

    pub fn phred_scores<'a>(&'a mut self, qual: &'a [u8]) -> Result<PhredScores<'a>, String> {
        Ok(PhredScores {
            orig_scores: qual,
            orig_format: self.fmt,
            phred: self.convert_to(qual, Phred)?,
        })
    }

    pub fn total_error(&mut self, qual: &[u8]) -> Result<f64, String> {
        Ok(self.phred_scores(qual)?.total_error())
    }
}

/// Converts between Phred scores (0-93) and ASCII formats
/// (Sanger = Sanger + Illumina 1.8+, Illumina = Illumina 1.3-1.7, Solexa)
#[inline(always)]
pub fn convert(q: &mut u8, from: QualFormat, to: QualFormat) {
    match from {
        Sanger => {
            match to {
                // min(): quality scores that cannot be represented in
                // Illumina 1.3-1.7/Solexa with an offset of 64 are silently truncated.
                // This seems acceptable, since it is just a loss of precision
                // (very low error probabilities cannot be represented).
                // Phred scores > 41 will anyway rarely occur.
                Illumina => *q = min(*q, 95) + 31,
                Phred => *q -= 33,
                Solexa => *q = phred_to_solexa(*q - 33),
                Sanger => {}
            }
        }
        Phred => match to {
            Sanger => *q = min(*q, 93) + 33,
            Illumina => *q = min(*q, 62) + 64,
            Solexa => *q = phred_to_solexa(min(*q, 62)),
            Phred => {}
        },
        Illumina => match to {
            Sanger => *q -= 31,
            Phred => *q -= 64,
            Solexa => *q = phred_to_solexa(*q - 64),
            Illumina => {}
        },
        Solexa => {
            let offset = match to {
                Sanger => 33,
                Illumina => 64,
                Phred => 0,
                Solexa => return,
            };
            *q = solexa_to_phred(*q) + offset;
        }
    }
}

/// Calculates the total estimated erorr in a sequence from Phred scores
/// They must already be within the range (0 - 93), so make sure to validate first
pub fn total_error(phred_qual: &[u8]) -> f64 {
    let mut prob = 0.;
    for &q in phred_qual {
        prob += phred_to_prob(q);
    }
    prob
}

/// Calculates the total estimated erorr in a sequence from Solexa ASCII
/// They must already be within the range (59 - 126), so make sure to validate first.
/// The round trip conversion of Solexa ASCII -> Phred (with convert()) and then
/// total_error() will lead to loss of precision due to rounding, therefore this
/// function exists separately.
pub fn total_error_solexa(solexa_ascii: &[u8]) -> f64 {
    let mut prob = 0.;
    for &q in solexa_ascii {
        prob += solexa_to_prob(q);
    }
    prob
}

#[inline]
fn guess_format(q: u8) -> Option<String> {
    let s = match q {
        0..=32 => None,
        33..=58 => Some(("Sanger/Illumina 1.8+", "'--fmt fq'")),
        59..=63 => Some((
            "Sanger/Illumina 1.8+ or eventually Solexa",
            "'--fmt fq' or '--fmt fq-solexa'",
        )),
        _ => None,
    };
    s.map(|(name, usage)| {
        format!(
            " It seems that the file is in the {} format. If so, use the option {}.",
            name, usage
        )
    })
}

/// Solexa ASCII (offset 64) to Phred score
/// 10 * log10(10^((q - 64)/10) + 1)
#[inline]
fn solexa_to_phred(q: u8) -> u8 {
    (10. * (10f64.powf((f64::from(q) - 64.) / 10.) + 1.).log10()).round() as u8
}

/// Phred score to Solexa ASCII
/// 10 * log10(10^(q/10) - 1) + 64
#[inline]
fn phred_to_solexa(q: u8) -> u8 {
    let s = ((10. * (10f64.powf(f64::from(q) / 10.) - 1.).log10()).round() + 64.) as u8;
    s.clamp(59, 126)
}

/// Estimated probability of error from Solexa ASCII
/// p = 1/(10^(Qs/10) + 1) where Qs is Solexa score (Solexa ASCII - 64)
/// the inverse: Qs = -10 * log10(p / (1 - p))
#[inline]
fn solexa_to_prob(q: u8) -> f64 {
    1f64 / (10f64.powf((f64::from(q) - 64.) / 10.) + 1.)
}

/// Estimated probability of error from Phred score
/// p = 10^(-Q/10)
/// the inverse would be: Q = -10 * log10(p)
#[inline]
pub fn phred_to_prob(q: u8) -> f64 {
    10f64.powf(-f64::from(q) / 10.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solexa_qual() {
        // according to https://www.ncbi.nlm.nih.gov/pmc/articles/PMC2847217/
        let qual = [1u8, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 10, 11];
        let solexa = [
            -5i8, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11,
        ];
        let solexa_back = [
            -5i8, -5, -5, -2, -2, 0, 0, 2, 2, 3, 3, 5, 6, 7, 8, 10, 10, 11,
        ];

        assert_eq!(phred_to_solexa(0), 59);

        for ((&q, &s), &sb) in qual[..].iter().zip(&solexa[..]).zip(&solexa_back[..]) {
            let s = (s + 64) as u8;
            let sb = (sb + 64) as u8;
            assert_eq!(solexa_to_phred(s), q);
            assert_eq!(phred_to_solexa(q), sb);
            assert_eq!(solexa_to_phred(phred_to_solexa(q)), q);
            assert_eq!(phred_to_solexa(solexa_to_phred(sb)), sb);
        }

        // these should be equal
        for q in 12..62 {
            assert_eq!(phred_to_solexa(q), q + 64);
        }
    }

    // e.g. http://www.somewhereville.com/2011/12/16/sanger-and-illumina-1-3-and-solexa-phred-score-q-ascii-glyph-base-error-conversion-tables/
    #[test]
    fn probs() {
        let mapping = [
            (0u8, 1f64),
            (1, 0.794_328_234_7),
            (10, 0.1),
            (40, 0.000_100_000_0),
            (93, 0.000_000_000_5),
        ];
        let f = 10f64.powi(10);
        for &(q, p) in &mapping[..] {
            approx::assert_relative_eq!((phred_to_prob(q) * f).round() / f, p);
        }
    }

    #[test]
    fn probs_solexa() {
        let mapping = [
            (-5i8, 0.759_746_9_f64),
            (0, 0.5),
            (1, 0.442_688_4),
            (10, 0.090_909_1),
            (40, 0.000_100_0),
            (62, 0.000_000_6),
        ];
        let f = 10f64.powi(7);
        for &(q, p) in &mapping[..] {
            approx::assert_relative_eq!((solexa_to_prob((q + 64) as u8) * f).round() / f, p);
        }
    }

    macro_rules! cnv {
        ($cnv:expr, $q:expr, $fmt:expr, $expected:expr) => {
            let res = $cnv.convert_to(&[$q], $fmt).map(|q| q[0]);
            assert_eq!(res, $expected);
        };
    }

    macro_rules! invalid {
        ($cnv:expr, $q:expr) => {
            assert!($cnv.validate(&[$q]).is_err());
        };
    }

    macro_rules! valid {
        ($cnv:expr, $q:expr) => {
            assert!($cnv.validate(&[$q]).is_ok());
        };
    }

    #[test]
    fn convert_sanger() {
        let mut q = QualConverter::new(Sanger);

        cnv!(q, 33, Sanger, Ok(33));
        cnv!(q, 43, Sanger, Ok(43));
        cnv!(q, 126, Sanger, Ok(126));

        cnv!(q, 33, Illumina, Ok(64));
        cnv!(q, 43, Illumina, Ok(74));
        cnv!(q, 95, Illumina, Ok(126));

        cnv!(q, 33, Phred, Ok(0));
        cnv!(q, 43, Phred, Ok(10));
        cnv!(q, 126, Phred, Ok(93));

        cnv!(q, 33, Solexa, Ok(59));
        cnv!(q, 43, Solexa, Ok(74));
        cnv!(q, 95, Solexa, Ok(126));

        // Out of range
        invalid!(q, 32);
        invalid!(q, 127);
    }

    #[test]
    fn convert_phred() {
        let mut q = QualConverter::new(Phred);

        cnv!(q, 0, Sanger, Ok(33));
        cnv!(q, 10, Sanger, Ok(43));
        cnv!(q, 93, Sanger, Ok(126));

        cnv!(q, 0, Phred, Ok(0));
        cnv!(q, 10, Phred, Ok(10));
        cnv!(q, 93, Phred, Ok(93));

        cnv!(q, 0, Illumina, Ok(64));
        cnv!(q, 10, Illumina, Ok(74));
        cnv!(q, 62, Illumina, Ok(126));

        cnv!(q, 0, Solexa, Ok(59));
        cnv!(q, 10, Solexa, Ok(74));
        cnv!(q, 62, Solexa, Ok(126));

        // Out of range (not possible)
        valid!(q, 0);
        valid!(q, 127);
        valid!(q, 255);
    }

    #[test]
    fn convert_illumina() {
        let mut q = QualConverter::new(Illumina);

        cnv!(q, 64, Sanger, Ok(33));
        cnv!(q, 74, Sanger, Ok(43));
        cnv!(q, 126, Sanger, Ok(95));

        cnv!(q, 64, Phred, Ok(0));
        cnv!(q, 74, Phred, Ok(10));
        cnv!(q, 126, Phred, Ok(62));

        cnv!(q, 64, Illumina, Ok(64));
        cnv!(q, 74, Illumina, Ok(74));
        cnv!(q, 126, Illumina, Ok(126));

        cnv!(q, 64, Solexa, Ok(59));
        cnv!(q, 74, Solexa, Ok(74));
        cnv!(q, 126, Solexa, Ok(126));

        // Out of range
        invalid!(q, 63);
        invalid!(q, 127);
    }

    #[test]
    fn convert_solexa() {
        let mut q = QualConverter::new(Solexa);

        cnv!(q, 59, Sanger, Ok(34));
        cnv!(q, 74, Sanger, Ok(43));
        cnv!(q, 126, Sanger, Ok(95));

        cnv!(q, 59, Phred, Ok(1));
        cnv!(q, 74, Phred, Ok(10));
        cnv!(q, 126, Phred, Ok(62));

        cnv!(q, 59, Illumina, Ok(65));
        cnv!(q, 74, Illumina, Ok(74));
        cnv!(q, 126, Illumina, Ok(126));

        cnv!(q, 59, Solexa, Ok(59));
        cnv!(q, 74, Solexa, Ok(74));
        cnv!(q, 126, Solexa, Ok(126));

        // Out of range
        invalid!(q, 58);
        invalid!(q, 127);
    }
}
