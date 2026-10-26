//! `username_hint` masking for the in-page dropdown (PRD §9):
//! `dimas@hotama.co.id` → `dim***@hotama.co.id`.

pub fn username_hint(username: &str) -> String {
    let (local, domain) = match username.rfind('@') {
        Some(i) => (&username[..i], Some(&username[i..])),
        None => (username, None),
    };
    let chars: Vec<char> = local.chars().collect();
    let keep = match chars.len() {
        0 => 0,
        1..=2 => 1,
        3..=4 => 2,
        _ => 3,
    };
    let mut out: String = chars.iter().take(keep).collect();
    out.push_str("***");
    if let Some(d) = domain {
        out.push_str(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::username_hint;

    #[test]
    fn masks() {
        assert_eq!(username_hint("dimas@hotama.co.id"), "dim***@hotama.co.id");
        assert_eq!(username_hint("198703122010011002"), "198***");
        assert_eq!(username_hint("ab"), "a***");
        assert_eq!(username_hint(""), "***");
        assert_eq!(username_hint("operator.prodi"), "ope***");
    }
}
