//! A small arithmetic language: numbers, the usual operators, a handful of
//! functions, and two constants. Integers stay exact as long as they fit
//! in an i128; anything else is an f64.

use std::fmt;

/// A number: an exact integer while it fits, a float otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Int(i128),
    Float(f64),
}

impl Value {
    fn as_f64(self) -> f64 {
        match self {
            // Precision is lost past 2^53, which is what a float is.
            #[allow(clippy::cast_precision_loss)]
            Value::Int(i) => i as f64,
            Value::Float(f) => f,
        }
    }

    /// Writes the value the way a person would: integers plain, floats with
    /// up to twelve significant digits and no trailing zeros.
    pub fn render(self) -> Result<String, Error> {
        match self {
            Value::Int(i) => Ok(i.to_string()),
            Value::Float(f) => render_float(f),
        }
    }
}

fn render_float(f: f64) -> Result<String, Error> {
    if !f.is_finite() {
        return Err(Error::NotANumber);
    }
    if f == 0.0 {
        return Ok("0".to_owned());
    }
    if f.fract() == 0.0 && f.abs() < 1e15 {
        return Ok(format!("{f:.0}"));
    }
    if f.abs() >= 1e21 || f.abs() < 1e-9 {
        return Ok(trim_zeros(&format!("{f:.11e}")));
    }
    // Twelve significant digits: the decimals that leaves room for.
    let magnitude = f.abs().log10().floor();
    let decimals = (11.0 - magnitude).clamp(0.0, 20.0);
    // `decimals` is in 0..=20, so the cast is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let decimals = decimals as usize;
    Ok(trim_zeros(&format!("{f:.decimals$}")))
}

/// Drops trailing zeros after a decimal point, in the mantissa when the
/// number is written with an exponent.
fn trim_zeros(text: &str) -> String {
    let (mantissa, exponent) = match text.find('e') {
        Some(at) => (&text[..at], &text[at..]),
        None => (text, ""),
    };
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    format!("{mantissa}{exponent}")
}

/// Why an expression could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Something that is not part of the language, at this character.
    Unexpected(String),
    /// The text ended while more was needed.
    Incomplete,
    DivideByZero,
    UnknownName(String),
    /// A function called with the wrong number of arguments.
    Arguments(&'static str, usize),
    /// The result was infinite or undefined.
    NotANumber,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unexpected(what) => write!(f, "unexpected {what}"),
            Error::Incomplete => write!(f, "incomplete expression"),
            Error::DivideByZero => write!(f, "division by zero"),
            Error::UnknownName(name) => write!(f, "unknown name {name}"),
            Error::Arguments(name, n) => write!(f, "{name} takes {n} argument(s)"),
            Error::NotANumber => write!(f, "result is not a number"),
        }
    }
}

/// Evaluates `text` as one expression.
pub fn evaluate(text: &str) -> Result<Value, Error> {
    let tokens = tokenize(text)?;
    let mut parser = Parser { tokens, at: 0 };
    let value = parser.expr()?;
    match parser.peek() {
        None => Ok(value),
        Some(token) => Err(Error::Unexpected(token.describe())),
    }
}

/// True when `text` is nothing but a number literal, with nothing to
/// compute.
pub fn is_literal(text: &str) -> bool {
    matches!(tokenize(text).as_deref(), Ok([Token::Number(_)]))
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(Value),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Open,
    Close,
    Comma,
}

impl Token {
    fn describe(&self) -> String {
        match self {
            Token::Number(v) => v.render().unwrap_or_else(|_| "number".to_owned()),
            Token::Name(n) => n.clone(),
            Token::Plus => "+".into(),
            Token::Minus => "-".into(),
            Token::Star => "*".into(),
            Token::Slash => "/".into(),
            Token::Percent => "%".into(),
            Token::Caret => "^".into(),
            Token::Open => "(".into(),
            Token::Close => ")".into(),
            Token::Comma => ",".into(),
        }
    }
}

fn tokenize(text: &str) -> Result<Vec<Token>, Error> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    tokens.push(Token::Caret);
                    i += 2;
                } else {
                    tokens.push(Token::Star);
                    i += 1;
                }
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            c if c.is_ascii_digit()
                || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) =>
            {
                let (value, next) = number(&chars, i)?;
                tokens.push(Token::Number(value));
                i = next;
            }
            c if c.is_alphabetic() => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Name(chars[start..i].iter().collect()));
            }
            other => return Err(Error::Unexpected(other.to_string())),
        }
    }
    Ok(tokens)
}

/// Reads a number literal starting at `start`; returns it and where it
/// ended. Handles `0x`, `0b` and `0o` prefixes, `_` between digits,
/// `1,000` thousands groups of exactly three digits, decimals and `1e6`.
fn number(chars: &[char], start: usize) -> Result<(Value, usize), Error> {
    let mut i = start;
    if chars[i] == '0' {
        let radix = match chars.get(i + 1) {
            Some('x' | 'X') => Some(16),
            Some('b' | 'B') => Some(2),
            Some('o' | 'O') => Some(8),
            _ => None,
        };
        if let Some(radix) = radix {
            i += 2;
            let mut value: i128 = 0;
            let mut digits = 0;
            while i < chars.len() {
                let c = chars[i];
                if c == '_' {
                    i += 1;
                    continue;
                }
                let Some(d) = c.to_digit(radix) else { break };
                value = value
                    .checked_mul(i128::from(radix))
                    .and_then(|v| v.checked_add(i128::from(d)))
                    .ok_or(Error::NotANumber)?;
                digits += 1;
                i += 1;
            }
            if digits == 0 {
                return Err(Error::Incomplete);
            }
            return Ok((Value::Int(value), i));
        }
    }
    let mut digits = String::new();
    let mut is_float = false;
    // The integer part, with `_` and thousands separators.
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            digits.push(c);
            i += 1;
        } else if (c == '_' && i > start && chars.get(i + 1).is_some_and(char::is_ascii_digit))
            || (c == ',' && thousands_group(chars, i))
        {
            i += 1;
        } else {
            break;
        }
    }
    if i < chars.len() && chars[i] == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
        is_float = true;
        digits.push('.');
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            digits.push(chars[i]);
            i += 1;
        }
    }
    if i < chars.len() && matches!(chars[i], 'e' | 'E') {
        let sign = matches!(chars.get(i + 1), Some('+' | '-'));
        let first = if sign { i + 2 } else { i + 1 };
        if chars.get(first).is_some_and(char::is_ascii_digit) {
            is_float = true;
            digits.push('e');
            if sign {
                digits.push(chars[i + 1]);
            }
            i = first;
            while i < chars.len() && chars[i].is_ascii_digit() {
                digits.push(chars[i]);
                i += 1;
            }
        }
    }
    if !is_float && let Ok(int) = digits.parse::<i128>() {
        return Ok((Value::Int(int), i));
    }
    digits
        .parse::<f64>()
        .map(|f| (Value::Float(f), i))
        .map_err(|_| Error::NotANumber)
}

/// A `,` at `at` is a thousands separator when exactly three digits follow
/// it and then something that is not a digit.
fn thousands_group(chars: &[char], at: usize) -> bool {
    at > 0
        && chars[at - 1].is_ascii_digit()
        && (1..=3).all(|k| chars.get(at + k).is_some_and(char::is_ascii_digit))
        && !chars.get(at + 4).is_some_and(char::is_ascii_digit)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        if token.is_some() {
            self.at += 1;
        }
        token
    }

    fn expr(&mut self) -> Result<Value, Error> {
        let mut left = self.term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.at += 1;
                    let right = self.term()?;
                    left = add(left, right);
                }
                Some(Token::Minus) => {
                    self.at += 1;
                    let right = self.term()?;
                    left = sub(left, right);
                }
                _ => return Ok(left),
            }
        }
    }

    fn term(&mut self) -> Result<Value, Error> {
        let mut left = self.unary()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.at += 1;
                    let right = self.unary()?;
                    left = mul(left, right);
                }
                Some(Token::Slash) => {
                    self.at += 1;
                    let right = self.unary()?;
                    left = div(left, right)?;
                }
                Some(Token::Percent) => {
                    self.at += 1;
                    let right = self.unary()?;
                    left = rem(left, right)?;
                }
                _ => return Ok(left),
            }
        }
    }

    fn unary(&mut self) -> Result<Value, Error> {
        match self.peek() {
            Some(Token::Minus) => {
                self.at += 1;
                Ok(neg(self.unary()?))
            }
            Some(Token::Plus) => {
                self.at += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    /// `a ^ b` binds tighter than unary minus and groups to the right, so
    /// `-2^2` is -4 and `2^3^2` is 512. The exponent may carry its own sign.
    fn power(&mut self) -> Result<Value, Error> {
        let base = self.postfix()?;
        if self.peek() == Some(&Token::Caret) {
            self.at += 1;
            let exponent = self.unary()?;
            return pow(base, exponent);
        }
        Ok(base)
    }

    /// `x%` is x divided by 100. Only where an operand ends: `50% * 2`,
    /// not `50 % 2`, which is the remainder.
    fn postfix(&mut self) -> Result<Value, Error> {
        let mut value = self.atom()?;
        while self.peek() == Some(&Token::Percent) && self.percent_is_postfix() {
            self.at += 1;
            value = div(value, Value::Int(100))?;
        }
        Ok(value)
    }

    fn percent_is_postfix(&self) -> bool {
        !matches!(
            self.tokens.get(self.at + 1),
            Some(Token::Number(_) | Token::Name(_) | Token::Open)
        )
    }

    fn atom(&mut self) -> Result<Value, Error> {
        match self.next() {
            None => Err(Error::Incomplete),
            Some(Token::Number(v)) => Ok(v),
            Some(Token::Open) => {
                let value = self.expr()?;
                match self.next() {
                    Some(Token::Close) => Ok(value),
                    None => Err(Error::Incomplete),
                    Some(other) => Err(Error::Unexpected(other.describe())),
                }
            }
            Some(Token::Name(name)) => {
                if self.peek() == Some(&Token::Open) {
                    self.at += 1;
                    let args = self.arguments()?;
                    call(&name, &args)
                } else {
                    match name.as_str() {
                        "pi" | "PI" => Ok(Value::Float(std::f64::consts::PI)),
                        "e" | "E" => Ok(Value::Float(std::f64::consts::E)),
                        _ => Err(Error::UnknownName(name)),
                    }
                }
            }
            Some(other) => Err(Error::Unexpected(other.describe())),
        }
    }

    fn arguments(&mut self) -> Result<Vec<Value>, Error> {
        let mut args = Vec::new();
        if self.peek() == Some(&Token::Close) {
            self.at += 1;
            return Ok(args);
        }
        loop {
            args.push(self.expr()?);
            match self.next() {
                Some(Token::Comma) => continue,
                Some(Token::Close) => return Ok(args),
                None => return Err(Error::Incomplete),
                Some(other) => return Err(Error::Unexpected(other.describe())),
            }
        }
    }
}

fn add(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x
            .checked_add(y)
            .map_or_else(|| Value::Float(a.as_f64() + b.as_f64()), Value::Int),
        _ => Value::Float(a.as_f64() + b.as_f64()),
    }
}

fn sub(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x
            .checked_sub(y)
            .map_or_else(|| Value::Float(a.as_f64() - b.as_f64()), Value::Int),
        _ => Value::Float(a.as_f64() - b.as_f64()),
    }
}

fn mul(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x
            .checked_mul(y)
            .map_or_else(|| Value::Float(a.as_f64() * b.as_f64()), Value::Int),
        _ => Value::Float(a.as_f64() * b.as_f64()),
    }
}

fn neg(a: Value) -> Value {
    match a {
        Value::Int(x) => x
            .checked_neg()
            .map_or_else(|| Value::Float(-a.as_f64()), Value::Int),
        Value::Float(f) => Value::Float(-f),
    }
}

fn div(a: Value, b: Value) -> Result<Value, Error> {
    match (a, b) {
        (_, Value::Int(0)) => Err(Error::DivideByZero),
        (Value::Float(_), Value::Float(y)) | (Value::Int(_), Value::Float(y)) if y == 0.0 => {
            Err(Error::DivideByZero)
        }
        (Value::Int(x), Value::Int(y)) => match x.checked_rem(y) {
            Some(0) => Ok(x
                .checked_div(y)
                .map_or_else(|| Value::Float(a.as_f64() / b.as_f64()), Value::Int)),
            _ => Ok(Value::Float(a.as_f64() / b.as_f64())),
        },
        _ => Ok(Value::Float(a.as_f64() / b.as_f64())),
    }
}

fn rem(a: Value, b: Value) -> Result<Value, Error> {
    match (a, b) {
        (_, Value::Int(0)) => Err(Error::DivideByZero),
        (Value::Int(x), Value::Int(y)) => Ok(x.checked_rem(y).map_or(Value::Int(0), Value::Int)),
        _ => {
            if b.as_f64() == 0.0 {
                return Err(Error::DivideByZero);
            }
            Ok(Value::Float(a.as_f64() % b.as_f64()))
        }
    }
}

fn pow(a: Value, b: Value) -> Result<Value, Error> {
    if let (Value::Int(x), Value::Int(y)) = (a, b)
        && let Ok(exp) = u32::try_from(y)
        && let Some(v) = x.checked_pow(exp)
    {
        return Ok(Value::Int(v));
    }
    finite(a.as_f64().powf(b.as_f64()))
}

fn finite(f: f64) -> Result<Value, Error> {
    if f.is_finite() {
        Ok(Value::Float(f))
    } else {
        Err(Error::NotANumber)
    }
}

fn call(name: &str, args: &[Value]) -> Result<Value, Error> {
    fn one(name: &'static str, args: &[Value]) -> Result<f64, Error> {
        match args {
            [a] => Ok(a.as_f64()),
            _ => Err(Error::Arguments(name, 1)),
        }
    }
    fn two(name: &'static str, args: &[Value]) -> Result<(Value, Value), Error> {
        match args {
            [a, b] => Ok((*a, *b)),
            _ => Err(Error::Arguments(name, 2)),
        }
    }
    match name {
        "sqrt" => finite(one("sqrt", args)?.sqrt()),
        "abs" => match args {
            [Value::Int(i)] => Ok(i
                .checked_abs()
                .map_or_else(|| Value::Float(args[0].as_f64().abs()), Value::Int)),
            _ => finite(one("abs", args)?.abs()),
        },
        "floor" | "ceil" | "round" => {
            let label: &'static str = match name {
                "floor" => "floor",
                "ceil" => "ceil",
                _ => "round",
            };
            match args {
                [Value::Int(i)] => Ok(Value::Int(*i)),
                _ => {
                    let x = one(label, args)?;
                    Ok(as_int_if_possible(match label {
                        "floor" => x.floor(),
                        "ceil" => x.ceil(),
                        _ => x.round(),
                    }))
                }
            }
        }
        "min" | "max" => {
            if args.is_empty() {
                return Err(Error::Arguments(
                    if name == "min" { "min" } else { "max" },
                    1,
                ));
            }
            let mut best = args[0];
            for &v in &args[1..] {
                let better = if name == "min" {
                    v.as_f64() < best.as_f64()
                } else {
                    v.as_f64() > best.as_f64()
                };
                if better {
                    best = v;
                }
            }
            Ok(best)
        }
        "sin" => finite(one("sin", args)?.sin()),
        "cos" => finite(one("cos", args)?.cos()),
        "tan" => finite(one("tan", args)?.tan()),
        "asin" => finite(one("asin", args)?.asin()),
        "acos" => finite(one("acos", args)?.acos()),
        "atan" => finite(one("atan", args)?.atan()),
        "log" => finite(one("log", args)?.log10()),
        "ln" => finite(one("ln", args)?.ln()),
        "log2" => finite(one("log2", args)?.log2()),
        "exp" => finite(one("exp", args)?.exp()),
        "pow" => {
            let (a, b) = two("pow", args)?;
            pow(a, b)
        }
        _ => Err(Error::UnknownName(name.to_owned())),
    }
}

/// A float with no fraction becomes an exact integer when it fits.
fn as_int_if_possible(f: f64) -> Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15 {
        // Whole and below 2^53, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation)]
        return Value::Int(f as i128);
    }
    Value::Float(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(text: &str) -> String {
        evaluate(text)
            .and_then(Value::render)
            .unwrap_or_else(|e| format!("error: {e}"))
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(eval("1 + 2 * 3"), "7");
        assert_eq!(eval("(1 + 2) * 3"), "9");
        assert_eq!(eval("2 ^ 3 ^ 2"), "512");
        assert_eq!(eval("2 ** 10"), "1024");
        assert_eq!(eval("-2 ^ 2"), "-4");
        assert_eq!(eval("2 ^ -1"), "0.5");
        assert_eq!(eval("10 % 3"), "1");
        assert_eq!(eval("--3"), "3");
    }

    #[test]
    fn integers_stay_exact_and_floats_are_tidy() {
        assert_eq!(eval("2 ** 62"), "4611686018427387904");
        assert_eq!(eval("0xFFFF_FFFF"), "4294967295");
        assert_eq!(eval("0b1010 + 0o17"), "25");
        assert_eq!(eval("1_000_000 + 1,000"), "1001000");
        assert_eq!(eval("10 / 4"), "2.5");
        assert_eq!(eval("10 / 5"), "2");
        assert_eq!(eval("1 / 3"), "0.333333333333");
        assert_eq!(eval("1e6 * 2"), "2000000");
        assert_eq!(eval("0.1 + 0.2"), "0.3");
        assert_eq!(eval(".5 * 4"), "2");
    }

    #[test]
    fn functions_constants_and_percent() {
        assert_eq!(eval("sqrt(16)"), "4");
        assert_eq!(eval("min(3, 1, 2) + max(1, 5)"), "6");
        assert_eq!(eval("round(2.5) + floor(1.9) + ceil(1.1)"), "6");
        assert_eq!(eval("abs(-7)"), "7");
        assert_eq!(eval("pow(2, 8)"), "256");
        assert_eq!(eval("log(1000)"), "3");
        assert_eq!(eval("ln(e)"), "1");
        assert_eq!(eval("cos(pi)"), "-1");
        assert_eq!(eval("20% * 50"), "10");
        assert_eq!(eval("200 * 15%"), "30");
        assert_eq!(eval("7 % 4"), "3");
    }

    #[test]
    fn errors_are_named() {
        assert_eq!(eval("1 / 0"), "error: division by zero");
        assert_eq!(eval("5 % 0"), "error: division by zero");
        assert_eq!(eval("foo + 1"), "error: unknown name foo");
        assert_eq!(eval("1 +"), "error: incomplete expression");
        assert_eq!(eval("hello world"), "error: unknown name hello");
        assert_eq!(eval("sqrt(1, 2)"), "error: sqrt takes 1 argument(s)");
        assert_eq!(eval("sqrt(-1)"), "error: result is not a number");
        assert_eq!(eval("1 $ 2"), "error: unexpected $");
    }

    #[test]
    fn literals_are_recognised() {
        assert!(is_literal("42"));
        assert!(is_literal(" 3.14 "));
        assert!(is_literal("0x1f"));
        assert!(!is_literal("-42"));
        assert!(!is_literal("1 + 1"));
    }
}
