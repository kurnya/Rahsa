//! Password generator (F-06). Uses the OS CSPRNG and uniform sampling.

use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use zeroize::Zeroizing;

use crate::model::GeneratorOptions;
use crate::{Error, Result};

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/~";
const AMBIGUOUS: &str = "Il1O0o|`'\"";

impl GeneratorOptions {
    pub fn validate(&self) -> Result<()> {
        if !(12..=64).contains(&self.length) {
            return Err(Error::Invalid("panjang password harus 12–64".into()));
        }
        if !(self.lower || self.upper || self.digits || self.symbols) {
            return Err(Error::Invalid("pilih minimal satu jenis karakter".into()));
        }
        let mins = [
            (self.lower, self.min_lower),
            (self.upper, self.min_upper),
            (self.digits, self.min_digits),
            (self.symbols, self.min_symbols),
        ];
        let total: u32 = mins.iter().filter(|(on, _)| *on).map(|(_, m)| *m).sum();
        if total > self.length {
            return Err(Error::Invalid("jumlah karakter wajib melebihi panjang password".into()));
        }
        if self.symbols {
            if let Some(set) = &self.symbol_set {
                if set.chars().filter(|c| !c.is_alphanumeric() && !c.is_whitespace()).count() == 0 {
                    return Err(Error::Invalid("daftar simbol kosong".into()));
                }
            }
        }
        Ok(())
    }
}

fn class(chars: &str, avoid_ambiguous: bool) -> Vec<char> {
    let mut v: Vec<char> = chars
        .chars()
        .filter(|c| !(avoid_ambiguous && AMBIGUOUS.contains(*c)))
        .collect();
    v.dedup();
    v
}

pub fn generate(opts: &GeneratorOptions) -> Result<Zeroizing<String>> {
    opts.validate()?;
    let symbols_src: String = match &opts.symbol_set {
        Some(s) => s.chars().filter(|c| !c.is_alphanumeric() && !c.is_whitespace()).collect(),
        None => SYMBOLS.to_string(),
    };
    let classes: Vec<(Vec<char>, u32)> = [
        (opts.lower, LOWER, opts.min_lower),
        (opts.upper, UPPER, opts.min_upper),
        (opts.digits, DIGITS, opts.min_digits),
        (opts.symbols, symbols_src.as_str(), opts.min_symbols),
    ]
    .into_iter()
    .filter(|(on, _, _)| *on)
    .map(|(_, chars, min)| (class(chars, opts.avoid_ambiguous), min))
    .filter(|(c, _)| !c.is_empty())
    .collect();

    let all: Vec<char> = classes.iter().flat_map(|(c, _)| c.iter().copied()).collect();
    let mut out: Vec<char> = Vec::with_capacity(opts.length as usize);
    for (chars, min) in &classes {
        for _ in 0..*min {
            out.push(*chars.choose(&mut OsRng).expect("non-empty class"));
        }
    }
    while out.len() < opts.length as usize {
        out.push(*all.choose(&mut OsRng).expect("non-empty alphabet"));
    }
    out.shuffle(&mut OsRng);
    let s = Zeroizing::new(out.iter().collect::<String>());
    out.iter_mut().for_each(|c| *c = '\0');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_length_and_rules() {
        let mut o = GeneratorOptions { length: 12, min_symbols: 2, min_digits: 3, ..Default::default() };
        for _ in 0..200 {
            let p = generate(&o).unwrap();
            assert_eq!(p.chars().count(), 12);
            assert!(p.chars().filter(|c| c.is_ascii_digit()).count() >= 3);
            assert!(p.chars().filter(|c| SYMBOLS.contains(*c)).count() >= 2);
            assert!(p.chars().any(|c| c.is_ascii_uppercase()));
            assert!(!p.chars().any(|c| AMBIGUOUS.contains(c)));
        }
        o.symbol_set = Some("#!".into());
        let p = generate(&o).unwrap();
        assert!(p.chars().filter(|c| !c.is_alphanumeric()).all(|c| c == '#' || c == '!'));
    }

    #[test]
    fn rejects_bad_options() {
        assert!(generate(&GeneratorOptions { length: 11, ..Default::default() }).is_err());
        assert!(generate(&GeneratorOptions { length: 65, ..Default::default() }).is_err());
        let none = GeneratorOptions { lower: false, upper: false, digits: false, symbols: false, ..Default::default() };
        assert!(generate(&none).is_err());
        assert!(generate(&GeneratorOptions { min_digits: 20, min_symbols: 20, ..Default::default() }).is_err());
    }

    #[test]
    fn outputs_differ() {
        let o = GeneratorOptions::default();
        assert_ne!(*generate(&o).unwrap(), *generate(&o).unwrap());
    }
}
