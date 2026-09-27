//! Closed ECMAScript regular-expression subset admission for schema `pattern` values
//! and `patternProperties` keys. Syntax only: nothing is compiled for matching and no
//! instance is ever evaluated. Identity escapes cover non-alphanumeric characters;
//! alphanumeric escapes must belong to the recognized set. Unescaped braces must form
//! valid quantifiers, character-class ranges must ascend, and inline flags, class
//! backreferences and unbounded numeric bounds are not admitted.

pub(super) fn valid(pattern: &str) -> bool {
    let mut p = Parser {
        chars: pattern.chars().collect(),
        i: 0,
    };
    p.disjunction().is_ok() && p.i == p.chars.len()
}

struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }
    fn at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.i + offset).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn disjunction(&mut self) -> Result<(), ()> {
        self.alternative()?;
        while self.eat('|') {
            self.alternative()?;
        }
        Ok(())
    }
    fn alternative(&mut self) -> Result<(), ()> {
        while self.peek().is_some_and(|c| c != '|' && c != ')') {
            self.term()?;
        }
        Ok(())
    }
    fn term(&mut self) -> Result<(), ()> {
        if self.assertion()? {
            // Assertions cannot be quantified.
            return Ok(());
        }
        self.atom()?;
        self.quantifier()
    }
    fn assertion(&mut self) -> Result<bool, ()> {
        match self.peek() {
            Some('^' | '$') => {
                self.i += 1;
                Ok(true)
            }
            Some('\\') if matches!(self.at(1), Some('b' | 'B')) => {
                self.i += 2;
                Ok(true)
            }
            Some('(') if self.at(1) == Some('?') => {
                let lookahead = matches!(self.at(2), Some('=' | '!'));
                let lookbehind = self.at(2) == Some('<') && matches!(self.at(3), Some('=' | '!'));
                if lookahead {
                    self.i += 3;
                } else if lookbehind {
                    self.i += 4;
                } else {
                    return Ok(false);
                }
                self.disjunction()?;
                if !self.eat(')') {
                    return Err(());
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn atom(&mut self) -> Result<(), ()> {
        match self.peek().ok_or(())? {
            '(' => self.group(),
            '[' => self.class(),
            '.' => {
                self.i += 1;
                Ok(())
            }
            '\\' => self.escape(false).map(|_| ()),
            // Nothing to repeat, or an unquantified brace.
            '*' | '+' | '?' | '{' | '}' => Err(()),
            _ => {
                self.i += 1;
                Ok(())
            }
        }
    }
    fn group(&mut self) -> Result<(), ()> {
        self.i += 1;
        if self.eat('?') && !self.eat(':') {
            if !self.eat('<') {
                // Inline flags and other extensions are not admitted.
                return Err(());
            }
            self.group_name()?;
        }
        self.disjunction()?;
        if !self.eat(')') {
            return Err(());
        }
        Ok(())
    }
    fn group_name(&mut self) -> Result<(), ()> {
        if self.word_run() == 0 || !self.eat('>') {
            return Err(());
        }
        Ok(())
    }
    fn word_run(&mut self) -> usize {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            self.i += 1;
        }
        self.i - start
    }
    fn quantifier(&mut self) -> Result<(), ()> {
        if self.eat('*') || self.eat('+') || self.eat('?') {
            self.eat('?');
            return Ok(());
        }
        if self.eat('{') {
            self.counted()?;
            self.eat('?');
        }
        Ok(())
    }
    fn counted(&mut self) -> Result<(), ()> {
        let lower = self.digits()?;
        let upper = if self.eat(',') {
            self.opt_digits()?
        } else {
            Some(lower)
        };
        if !self.eat('}') {
            return Err(());
        }
        match upper {
            Some(upper) if upper < lower => Err(()),
            _ => Ok(()),
        }
    }
    fn digits(&mut self) -> Result<u128, ()> {
        let mut value: u128 = 0;
        let mut seen = false;
        while let Some(c) = self.peek().filter(|c| c.is_ascii_digit()) {
            self.i += 1;
            seen = true;
            value = value
                .checked_mul(10)
                .and_then(|v| v.checked_add(u128::from(c) - u128::from('0')))
                .ok_or(())?;
        }
        seen.then_some(value).ok_or(())
    }
    fn opt_digits(&mut self) -> Result<Option<u128>, ()> {
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.digits().map(Some)
        } else {
            Ok(None)
        }
    }
    fn escape(&mut self, in_class: bool) -> Result<Option<char>, ()> {
        self.i += 1;
        let Some(c) = self.peek() else { return Err(()) };
        self.i += 1;
        match c {
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => Ok(None),
            'b' | 'B' if in_class => Ok(Some('\u{8}')),
            'f' => Ok(Some('\u{c}')),
            'n' => Ok(Some('\n')),
            'r' => Ok(Some('\r')),
            't' => Ok(Some('\t')),
            'v' => Ok(Some('\u{b}')),
            'p' | 'P' => self.property(),
            'k' if !in_class => {
                if !self.eat('<') {
                    return Err(());
                }
                self.group_name()?;
                Ok(None)
            }
            'x' => self.fixed_hex(2),
            'u' => {
                if self.eat('{') {
                    self.braced_hex()
                } else {
                    self.fixed_hex(4)
                }
            }
            'c' => {
                if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    self.i += 1;
                    Ok(Some((self.chars[self.i - 1] as u8 % 32) as char))
                } else {
                    Err(())
                }
            }
            '0' => Ok(Some('\0')),
            '1'..='9' if !in_class => {
                // Backreference; its numeric identity is not resolved here.
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
                Ok(None)
            }
            c if c.is_ascii_alphanumeric() => Err(()),
            _ => Ok(Some(c)),
        }
    }
    fn fixed_hex(&mut self, count: usize) -> Result<Option<char>, ()> {
        let mut value: u32 = 0;
        for _ in 0..count {
            let c = self.peek().filter(|c| c.is_ascii_hexdigit()).ok_or(())?;
            self.i += 1;
            value = value * 16 + c.to_digit(16).ok_or(())?;
        }
        Ok(Some(char::from_u32(value).ok_or(())?))
    }
    fn braced_hex(&mut self) -> Result<Option<char>, ()> {
        let mut value: u32 = 0;
        let mut seen = false;
        while let Some(c) = self.peek().filter(|c| c.is_ascii_hexdigit()) {
            self.i += 1;
            seen = true;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(c.to_digit(16).ok_or(()).ok()?))
                .ok_or(())?;
        }
        if !seen || !self.eat('}') {
            return Err(());
        }
        Ok(Some(char::from_u32(value).ok_or(())?))
    }
    fn property(&mut self) -> Result<Option<char>, ()> {
        if !self.eat('{') {
            return Err(());
        }
        if self.property_name() == 0 || (self.eat('=') && self.property_name() == 0) {
            return Err(());
        }
        if !self.eat('}') {
            return Err(());
        }
        Ok(None)
    }
    fn property_name(&mut self) -> usize {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            self.i += 1;
        }
        self.i - start
    }
    fn class(&mut self) -> Result<(), ()> {
        self.i += 1;
        self.eat('^');
        loop {
            match self.peek() {
                None => return Err(()),
                Some(']') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => {}
            }
            let lower = self.class_atom()?;
            if self.peek() == Some('-') && self.at(1).is_some_and(|c| c != ']') {
                self.i += 1;
                let upper = self.class_atom()?;
                if !matches!((lower, upper), (Some(lower), Some(upper)) if lower <= upper) {
                    return Err(());
                }
            }
        }
    }
    fn class_atom(&mut self) -> Result<Option<char>, ()> {
        match self.peek().ok_or(())? {
            '\\' => self.escape(true),
            '-' => {
                self.i += 1;
                Ok(Some('-'))
            }
            _ => {
                let c = self.chars[self.i];
                self.i += 1;
                Ok(Some(c))
            }
        }
    }
}
