//! Evaluates the games' count/duration formulas to a base value.
//!
//! Formulas look like `10-5*WorkerPar("perk_heat_saver")`: arithmetic over
//! numbers and calls that read game state. Calls evaluate to 0, which gives
//! the value without perks or bonuses.

pub fn eval(src: &str) -> Option<f64> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }
    if let Ok(v) = src.parse::<f64>() {
        return Some(v);
    }
    let mut p = Parser { s: src.as_bytes(), pos: 0 };
    let v = p.expr()?;
    p.ws();
    (p.pos == p.s.len() && v.is_finite()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.pos).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.pos += 1;
            let r = self.term()?;
            v = if op == b'+' { v + r } else { v - r };
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.pos += 1;
            let r = self.factor()?;
            v = if op == b'*' { v * r } else { v / r };
        }
        Some(v)
    }

    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            b'-' => {
                self.pos += 1;
                Some(-self.factor()?)
            }
            b'(' => {
                self.pos += 1;
                let v = self.expr()?;
                (self.peek()? == b')').then(|| self.pos += 1)?;
                Some(v)
            }
            c if c.is_ascii_digit() || c == b'.' => {
                let start = self.pos;
                while self.pos < self.s.len() && (self.s[self.pos].is_ascii_digit() || self.s[self.pos] == b'.') {
                    self.pos += 1;
                }
                std::str::from_utf8(&self.s[start..self.pos]).ok()?.parse().ok()
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                while self.pos < self.s.len() && (self.s[self.pos].is_ascii_alphanumeric() || b"_.".contains(&self.s[self.pos])) {
                    self.pos += 1;
                }
                if self.peek() == Some(b'(') {
                    self.skip_call()?;
                }
                Some(0.0)
            }
            _ => None,
        }
    }

    /// Skips a balanced `(...)`, including quoted strings.
    fn skip_call(&mut self) -> Option<()> {
        let mut depth = 0usize;
        let mut quoted = false;
        while let Some(&c) = self.s.get(self.pos) {
            self.pos += 1;
            match c {
                b'"' => quoted = !quoted,
                b'(' if !quoted => depth += 1,
                b')' if !quoted => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(());
                    }
                }
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::eval;

    #[test]
    fn numbers_and_arithmetic() {
        assert_eq!(eval("3"), Some(3.0));
        assert_eq!(eval(" 2.5 "), Some(2.5));
        assert_eq!(eval("2+3*4"), Some(14.0));
        assert_eq!(eval("(2+3)*4"), Some(20.0));
        assert_eq!(eval("-1+3"), Some(2.0));
        assert_eq!(eval(""), None);
    }

    #[test]
    fn calls_evaluate_to_zero() {
        assert_eq!(eval(r#"10-5*WorkerPar("perk_heat_saver")"#), Some(10.0));
        assert_eq!(eval(r#"20-10*WorkerPar("a(b)")"#), Some(20.0));
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(eval("2+"), None);
        assert_eq!(eval("2 3"), None);
    }
}
