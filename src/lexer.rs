#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Num(f64, Option<NumberFormat>),
    Sym(String),
    Plu,
    Min,
    Str,
    Sle,
    Per,
    Hat,
    Eqa,
    Com,
    LPa,
    RPa,
    EOE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    Scientific,
    Binary,
    Hexadecimal,
    Unit(UnitFormat),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitFormat {
    LowerB,
    UpperB,
}

impl TokenKind {
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Num(_, _) => "number",
            TokenKind::Sym(_) => "symbol",
            TokenKind::Plu => "`+`",
            TokenKind::Min => "`-`",
            TokenKind::Str => "`*`",
            TokenKind::Sle => "`/`",
            TokenKind::Per => "`%`",
            TokenKind::Hat => "`^`",
            TokenKind::Eqa => "`=`",
            TokenKind::Com => "`,`",
            TokenKind::LPa => "`(`",
            TokenKind::RPa => "`)`",
            TokenKind::EOE => "end of expression",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

impl Token {
    fn new(kind: TokenKind, pos: usize) -> Self {
        Self { kind, pos }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexingError {
    UnexpectedCharacter { ch: char, pos: usize },

    InvalidNumber { text: String, pos: usize },
}

#[derive(Default)]
pub struct Lexer {
    input_len: usize,
    chars: Vec<(usize, char)>,
    current: usize,
}

impl Lexer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tokenize(&mut self, input: &str) -> Result<Vec<Token>, LexingError> {
        self.input_len = input.len();
        self.chars = input.char_indices().collect();
        self.current = 0;

        let mut tokens = Vec::new();

        while let Some((pos, ch)) = self.peek() {
            match ch {
                c if c.is_whitespace() => {
                    self.next();
                }

                '0' if matches!(self.peek_next(), Some((_, 'x' | 'X'))) => {
                    let token = self.lex_prefixed_number(16, 2)?;
                    tokens.push(token);
                }

                'b' if matches!(self.peek_next(), Some((_, '0' | '1'))) => {
                    let token = self.lex_prefixed_number(2, 1)?;
                    tokens.push(token);
                }

                '0'..='9' | '.' => {
                    let token = self.lex_number()?;
                    tokens.push(token);
                }

                'a'..='z' | 'A'..='Z' | '_' => {
                    let token = self.lex_symbol()?;
                    tokens.push(token);
                }

                '+' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Plu, pos));
                }

                '-' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Min, pos));
                }

                '*' if matches!(self.peek_next(), Some((_, '*'))) => {
                    self.next();
                    self.next();
                    tokens.push(Token::new(TokenKind::Hat, pos));
                }

                '*' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Str, pos));
                }

                '/' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Sle, pos));
                }

                '^' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Hat, pos));
                }

                '%' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Per, pos));
                }

                '=' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Eqa, pos));
                }

                ',' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::Com, pos));
                }

                '(' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::LPa, pos));
                }

                ')' => {
                    self.next();
                    tokens.push(Token::new(TokenKind::RPa, pos));
                }

                _ => {
                    return Err(LexingError::UnexpectedCharacter { ch, pos });
                }
            }
        }

        tokens.push(Token::new(TokenKind::EOE, self.input_len));

        Ok(tokens)
    }

    fn lex_number(&mut self) -> Result<Token, LexingError> {
        let start = self.position();

        let mut text = String::new();
        let mut dot_seen = false;
        let mut exp_seen = false;
        let mut format = None;

        while let Some((_, ch)) = self.peek() {
            match ch {
                '0'..='9' => {
                    text.push(ch);
                    self.next();
                }

                '.' if !dot_seen => {
                    dot_seen = true;
                    text.push(ch);
                    self.next();
                }

                'e' | 'E' if !exp_seen => {
                    exp_seen = true;
                    text.push(ch);
                    self.next();

                    if let Some((_, sign @ ('+' | '-'))) = self.peek() {
                        text.push(sign);
                        self.next();
                    }

                    match self.peek() {
                        Some((_, '0'..='9')) => {}
                        _ => {
                            return Err(LexingError::InvalidNumber { text, pos: start });
                        }
                    }
                }

                '.' => {
                    return Err(LexingError::InvalidNumber { text, pos: start });
                }

                _ => break,
            }
        }

        if text == "." {
            return Err(LexingError::InvalidNumber { text, pos: start });
        }

        let mut value = text
            .parse::<f64>()
            .map_err(|_| LexingError::InvalidNumber {
                text: text.clone(),
                pos: start,
            })?;

        if exp_seen {
            format = Some(NumberFormat::Scientific);
        }

        if let Some(unit) = self.lex_unit_suffix() {
            value *= unit.multiplier;
            format = Some(NumberFormat::Unit(unit.format));
        }

        Ok(Token::new(TokenKind::Num(value, format), start))
    }

    fn lex_prefixed_number(&mut self, radix: u32, prefix_len: usize) -> Result<Token, LexingError> {
        let start = self.position();
        let mut text = String::new();

        for _ in 0..prefix_len {
            if let Some((_, ch)) = self.next() {
                text.push(ch);
            }
        }

        let digits_start = text.len();
        while let Some((_, ch)) = self.peek() {
            if ch.is_digit(radix) {
                text.push(ch);
                self.next();
            } else {
                break;
            }
        }

        if text.len() == digits_start {
            return Err(LexingError::InvalidNumber { text, pos: start });
        }

        if matches!(self.peek(), Some((_, ch)) if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
        {
            while let Some((_, ch)) = self.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                    text.push(ch);
                    self.next();
                } else {
                    break;
                }
            }

            return Err(LexingError::InvalidNumber { text, pos: start });
        }

        let digits = &text[digits_start..];
        let value = u64::from_str_radix(digits, radix)
            .map(|value| value as f64)
            .map_err(|_| LexingError::InvalidNumber {
                text: text.clone(),
                pos: start,
            })?;

        Ok(Token::new(TokenKind::Num(value, None), start))
    }

    fn lex_unit_suffix(&mut self) -> Option<UnitSuffix> {
        let start = self.current;
        let mut text = String::new();

        while let Some((_, ch)) = self.peek() {
            if ch.is_ascii_alphabetic() {
                text.push(ch);
                self.next();
            } else {
                break;
            }
        }

        let suffix = UnitSuffix::parse(&text);
        if suffix.is_none() {
            self.current = start;
        }

        suffix
    }

    fn lex_symbol(&mut self) -> Result<Token, LexingError> {
        let start = self.position();

        let mut text = String::new();

        while let Some((_, ch)) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                text.push(ch);
                self.next();
            } else {
                break;
            }
        }

        Ok(Token::new(TokenKind::Sym(text), start))
    }

    fn next(&mut self) -> Option<(usize, char)> {
        let next = self.peek();
        self.current += usize::from(next.is_some());
        next
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.current).copied()
    }

    fn peek_next(&self) -> Option<(usize, char)> {
        self.chars.get(self.current + 1).copied()
    }

    fn position(&self) -> usize {
        self.peek().map(|(pos, _)| pos).unwrap_or(self.input_len)
    }
}

struct UnitSuffix {
    multiplier: f64,
    format: UnitFormat,
}

impl UnitSuffix {
    fn parse(text: &str) -> Option<Self> {
        let (format, power) = match text {
            "b" => (UnitFormat::LowerB, 0),
            "Kb" => (UnitFormat::LowerB, 1),
            "Mb" => (UnitFormat::LowerB, 2),
            "Gb" => (UnitFormat::LowerB, 3),
            "Tb" => (UnitFormat::LowerB, 4),
            "Pb" => (UnitFormat::LowerB, 5),
            "Eb" => (UnitFormat::LowerB, 6),
            "B" => (UnitFormat::UpperB, 0),
            "KB" => (UnitFormat::UpperB, 1),
            "MB" => (UnitFormat::UpperB, 2),
            "GB" => (UnitFormat::UpperB, 3),
            "TB" => (UnitFormat::UpperB, 4),
            "PB" => (UnitFormat::UpperB, 5),
            "EB" => (UnitFormat::UpperB, 6),
            _ => return None,
        };

        Some(Self {
            multiplier: 1024.0_f64.powi(power),
            format,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, LexingError, TokenKind};

    #[test]
    fn tokenizes_expression() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("1 + 2.5 * (3 - .5)").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Num(1.0, None),
                TokenKind::Plu,
                TokenKind::Num(2.5, None),
                TokenKind::Str,
                TokenKind::LPa,
                TokenKind::Num(3.0, None),
                TokenKind::Min,
                TokenKind::Num(0.5, None),
                TokenKind::RPa,
                TokenKind::EOE,
            ]
        );
    }

    #[test]
    fn tokenizes_double_asterisk_as_power() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("2 ** 3").unwrap();

        assert_eq!(tokens[1].kind, TokenKind::Hat);
    }

    #[test]
    fn reuses_lexer_state_between_inputs() {
        let mut lexer = Lexer::new();

        let first = lexer.tokenize("10").unwrap();
        let second = lexer.tokenize("2 + 3").unwrap();

        assert_eq!(first[0].kind, TokenKind::Num(10.0, None));
        assert_eq!(second[0].kind, TokenKind::Num(2.0, None));
        assert_eq!(second[1].kind, TokenKind::Plu);
        assert_eq!(second[2].kind, TokenKind::Num(3.0, None));
    }

    #[test]
    fn lex_string_correctly() {
        let mut lexer = Lexer::new();

        let first = lexer.tokenize("1 + ans").unwrap();
        let second = lexer.tokenize("ans * 2").unwrap();

        assert_eq!(first[2].kind, TokenKind::Sym("ans".into()));
        assert_eq!(second[0].kind, TokenKind::Sym("ans".into()));
    }

    #[test]
    fn tokenizes_scientific_numbers() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("10e5 + 34e-6").unwrap();

        assert_eq!(
            tokens[0].kind,
            TokenKind::Num(1_000_000.0, Some(crate::lexer::NumberFormat::Scientific))
        );
        assert_eq!(
            tokens[2].kind,
            TokenKind::Num(0.000034, Some(crate::lexer::NumberFormat::Scientific))
        );
    }

    #[test]
    fn tokenizes_binary_numbers() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("b01010101 + b10001011").unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Num(85.0, None));
        assert_eq!(tokens[2].kind, TokenKind::Num(139.0, None));
    }

    #[test]
    fn tokenizes_hexadecimal_numbers() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("0xAFC34 + 0xAfcb3D").unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Num(719_924.0, None));
        assert_eq!(tokens[2].kind, TokenKind::Num(11_520_829.0, None));
    }

    #[test]
    fn rejects_invalid_prefixed_numbers() {
        let mut lexer = Lexer::new();

        let err = lexer.tokenize("b102").unwrap_err();
        assert!(matches!(
            err,
            LexingError::InvalidNumber { text, pos } if text == "b102" && pos == 0
        ));

        let err = lexer.tokenize("0x").unwrap_err();
        assert!(matches!(
            err,
            LexingError::InvalidNumber { text, pos } if text == "0x" && pos == 0
        ));

        let err = lexer.tokenize("0xAFG").unwrap_err();
        assert!(matches!(
            err,
            LexingError::InvalidNumber { text, pos } if text == "0xAFG" && pos == 0
        ));
    }

    #[test]
    fn tokenizes_unit_numbers() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("2Kb + 3MB").unwrap();

        assert_eq!(
            tokens[0].kind,
            TokenKind::Num(
                2048.0,
                Some(crate::lexer::NumberFormat::Unit(
                    crate::lexer::UnitFormat::LowerB
                ))
            )
        );
        assert_eq!(
            tokens[2].kind,
            TokenKind::Num(
                3_145_728.0,
                Some(crate::lexer::NumberFormat::Unit(
                    crate::lexer::UnitFormat::UpperB
                ))
            )
        );
    }
}
