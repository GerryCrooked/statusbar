//! Tiny case-insensitive wildcard matcher (`*` = any run, `?` = one character).
//! Dependency-free on purpose; used for host targets and monitoring rules.

pub fn matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::matches;
    #[test]
    fn literals_and_case() {
        assert!(matches("mail01", "MAIL01"));
        assert!(!matches("mail01", "mail02"));
        assert!(matches("", ""));
        assert!(!matches("", "x"));
    }
    #[test]
    fn wildcards() {
        assert!(matches("*", ""));
        assert!(matches("*", "anything"));
        assert!(matches("mail*/smtp*", "mail01.example.local/SMTP check"));
        assert!(matches("*exchange*", "srv-exchange-02/Service"));
        assert!(matches("pc-acc-??", "PC-ACC-07"));
        assert!(!matches("pc-acc-??", "PC-ACC-007"));
        assert!(matches("a*b*c", "aXXbYYbZZc"));
        assert!(!matches("a*b*c", "aXXbYY"));
        assert!(matches("*.example.local/*", "db1.example.local/"));
    }
}
