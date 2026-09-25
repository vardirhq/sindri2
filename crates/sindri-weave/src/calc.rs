//! `calc()`: arithmetic on lengths, as CSS does it.
//!
//! Lengths can be added and subtracted, and multiplied or divided by plain
//! numbers, so whatever a `calc()` says reduces to so many overlay units, so
//! many pixels, so much of the viewport's width and height, and so much of
//! the containing box. That mix is what is kept, and it is resolved when the
//! viewport and the box are known, exactly as a single length is.
//!
//! `min()`, `max()` and `clamp()` are not linear and are not read yet.

/// A length made of several units at once: what a `calc()` comes to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Mix {
    pub overlay: f32,
    pub pixels: f32,
    pub view_width: f32,
    pub view_height: f32,
    pub percent: f32,
    /// Whether any part is a percentage, which then needs a box to resolve.
    pub relative: bool,
}

impl Mix {
    fn scaled(self, by: f32) -> Self {
        Self {
            overlay: self.overlay * by,
            pixels: self.pixels * by,
            view_width: self.view_width * by,
            view_height: self.view_height * by,
            percent: self.percent * by,
            relative: self.relative,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            overlay: self.overlay + other.overlay,
            pixels: self.pixels + other.pixels,
            view_width: self.view_width + other.view_width,
            view_height: self.view_height + other.view_height,
            percent: self.percent + other.percent,
            relative: self.relative || other.relative,
        }
    }
}

/// One side of an operation: a plain number, or a length.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Term {
    Number(f32),
    Length(Mix),
}

/// `calc(...)`, read into its mix of units. `None` for anything CSS would
/// not accept: an unknown unit, two lengths multiplied, division by a length
/// or by zero, or a bare number where a length belongs.
pub(crate) fn parse(text: &str) -> Option<Mix> {
    let inner = text.trim().strip_prefix("calc(")?.strip_suffix(')')?;
    let tokens = tokenize(inner)?;
    let mut position = 0;
    let term = sum(&tokens, &mut position)?;
    if position != tokens.len() {
        return None;
    }
    match term {
        Term::Length(mix) => Some(mix),
        // `calc(0)` is a length of nothing, as a bare `0` is.
        Term::Number(0.0) => Some(Mix::default()),
        Term::Number(_) => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Value(Term),
    Plus,
    Minus,
    Times,
    Divide,
    Open,
    Close,
}

fn tokenize(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        match c {
            _ if c.is_whitespace() => at += 1,
            '+' | '-' if starts_number(&chars, at, &tokens) => {
                let (term, next) = value(&chars, at)?;
                tokens.push(Token::Value(term));
                at = next;
            }
            '+' => {
                tokens.push(Token::Plus);
                at += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                at += 1;
            }
            '*' => {
                tokens.push(Token::Times);
                at += 1;
            }
            '/' => {
                tokens.push(Token::Divide);
                at += 1;
            }
            '(' => {
                tokens.push(Token::Open);
                at += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                at += 1;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let (term, next) = value(&chars, at)?;
                tokens.push(Token::Value(term));
                at = next;
            }
            // A nested `calc(` is only parentheses.
            'c' if chars[at..].starts_with(&['c', 'a', 'l', 'c', '(']) => {
                tokens.push(Token::Open);
                at += 5;
            }
            _ => return None,
        }
    }
    Some(tokens)
}

/// Whether a sign at `at` belongs to a number (`-2px`) rather than being an
/// operator: it does when nothing, an operator or `(` is before it and a
/// digit follows. CSS needs spaces around `+` and `-` operators, so `a -b`
/// cannot be read any other way.
fn starts_number(chars: &[char], at: usize, tokens: &[Token]) -> bool {
    let operand_expected = !matches!(tokens.last(), Some(Token::Value(_) | Token::Close));
    operand_expected
        && chars
            .get(at + 1)
            .is_some_and(|next| next.is_ascii_digit() || *next == '.')
}

/// A number and its unit, starting at `at`.
fn value(chars: &[char], at: usize) -> Option<(Term, usize)> {
    let mut end = at;
    if matches!(chars.get(end), Some('+' | '-')) {
        end += 1;
    }
    while chars
        .get(end)
        .is_some_and(|c| c.is_ascii_digit() || *c == '.')
    {
        end += 1;
    }
    let number: f32 = chars[at..end].iter().collect::<String>().parse().ok()?;
    if !number.is_finite() {
        return None;
    }
    let unit_start = end;
    while chars
        .get(end)
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == '%')
    {
        end += 1;
    }
    let unit: String = chars[unit_start..end].iter().collect();
    let length = |mix: Mix| Term::Length(mix);
    let term = match unit.as_str() {
        "" => Term::Number(number),
        "px" => length(Mix {
            pixels: number,
            ..Mix::default()
        }),
        "vw" => length(Mix {
            view_width: number,
            ..Mix::default()
        }),
        "vh" => length(Mix {
            view_height: number,
            ..Mix::default()
        }),
        "%" => length(Mix {
            percent: number,
            relative: true,
            ..Mix::default()
        }),
        // The overlay's own unit, written out: `calc(0.5u + 10px)`.
        "u" => length(Mix {
            overlay: number,
            ..Mix::default()
        }),
        _ => return None,
    };
    Some((term, end))
}

fn sum(tokens: &[Token], at: &mut usize) -> Option<Term> {
    let mut total = product(tokens, at)?;
    while let Some(sign @ (Token::Plus | Token::Minus)) = tokens.get(*at) {
        let negative = *sign == Token::Minus;
        *at += 1;
        let next = product(tokens, at)?;
        let next = if negative { scale(next, -1.0) } else { next };
        total = match (total, next) {
            (Term::Number(a), Term::Number(b)) => Term::Number(a + b),
            (Term::Length(a), Term::Length(b)) => Term::Length(a.plus(b)),
            // CSS will not add a number to a length.
            _ => return None,
        };
    }
    Some(total)
}

fn product(tokens: &[Token], at: &mut usize) -> Option<Term> {
    let mut total = operand(tokens, at)?;
    while let Some(op @ (Token::Times | Token::Divide)) = tokens.get(*at) {
        let dividing = *op == Token::Divide;
        *at += 1;
        let next = operand(tokens, at)?;
        total = match (total, next, dividing) {
            (Term::Number(a), Term::Number(b), false) => Term::Number(a * b),
            (Term::Number(a), Term::Number(b), true) if b != 0.0 => Term::Number(a / b),
            (Term::Length(a), Term::Number(b), false)
            | (Term::Number(b), Term::Length(a), false) => Term::Length(a.scaled(b)),
            (Term::Length(a), Term::Number(b), true) if b != 0.0 => Term::Length(a.scaled(1.0 / b)),
            _ => return None,
        };
    }
    Some(total)
}

fn operand(tokens: &[Token], at: &mut usize) -> Option<Term> {
    match tokens.get(*at)? {
        Token::Value(term) => {
            *at += 1;
            Some(*term)
        }
        Token::Open => {
            *at += 1;
            let inner = sum(tokens, at)?;
            (tokens.get(*at) == Some(&Token::Close)).then(|| {
                *at += 1;
                inner
            })
        }
        _ => None,
    }
}

fn scale(term: Term, by: f32) -> Term {
    match term {
        Term::Number(number) => Term::Number(number * by),
        Term::Length(mix) => Term::Length(mix.scaled(by)),
    }
}

#[cfg(test)]
mod tests {
    use super::{Mix, parse};

    #[test]
    fn lengths_add_and_scale_as_css_does() {
        let mix = parse("calc(100% - 2 * 20px + 5vw)").expect("a length");
        assert_eq!(
            mix,
            Mix {
                percent: 100.0,
                pixels: -40.0,
                view_width: 5.0,
                relative: true,
                ..Mix::default()
            }
        );
        let halved = parse("calc((50vh + 10px) / 2)").expect("a length");
        assert_eq!(halved.view_height, 25.0);
        assert_eq!(halved.pixels, 5.0);
        assert!(!halved.relative);
        assert_eq!(
            parse("calc(-10px + 30px)").map(|mix| mix.pixels),
            Some(20.0)
        );
        assert_eq!(
            parse("calc(calc(10px) * 3)").map(|mix| mix.pixels),
            Some(30.0)
        );
    }

    #[test]
    fn what_css_refuses_is_refused() {
        assert_eq!(parse("calc(10px * 10px)"), None);
        assert_eq!(parse("calc(10px / 0)"), None);
        assert_eq!(parse("calc(10px / 2px)"), None);
        assert_eq!(parse("calc(10px + 2)"), None);
        assert_eq!(parse("calc(3)"), None);
        assert_eq!(parse("calc(10em)"), None);
        assert_eq!(parse("calc(10px"), None);
        assert_eq!(parse("calc(0)"), Some(Mix::default()));
    }
}
