#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Num(f64),
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

impl TokenKind {
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Num(_) => "number",
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

                '.' => {
                    return Err(LexingError::InvalidNumber { text, pos: start });
                }

                _ => break,
            }
        }

        if text == "." {
            return Err(LexingError::InvalidNumber { text, pos: start });
        }

        let value = text
            .parse::<f64>()
            .map_err(|_| LexingError::InvalidNumber {
                text: text.clone(),
                pos: start,
            })?;

        Ok(Token::new(TokenKind::Num(value), start))
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

    fn position(&self) -> usize {
        self.peek().map(|(pos, _)| pos).unwrap_or(self.input_len)
    }
}

#[cfg(test)]
mod tests {
    use crate::lexer::{Lexer, TokenKind};

    #[test]
    fn tokenizes_expression() {
        let mut lexer = Lexer::new();

        let tokens = lexer.tokenize("1 + 2.5 * (3 - .5)").unwrap();
        let kinds: Vec<_> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Num(1.0),
                TokenKind::Plu,
                TokenKind::Num(2.5),
                TokenKind::Str,
                TokenKind::LPa,
                TokenKind::Num(3.0),
                TokenKind::Min,
                TokenKind::Num(0.5),
                TokenKind::RPa,
                TokenKind::EOE,
            ]
        );
    }

    #[test]
    fn reuses_lexer_state_between_inputs() {
        let mut lexer = Lexer::new();

        let first = lexer.tokenize("10").unwrap();
        let second = lexer.tokenize("2 + 3").unwrap();

        assert_eq!(first[0].kind, TokenKind::Num(10.0));
        assert_eq!(second[0].kind, TokenKind::Num(2.0));
        assert_eq!(second[1].kind, TokenKind::Plu);
        assert_eq!(second[2].kind, TokenKind::Num(3.0));
    }

    #[test]
    fn lex_string_correctly() {
        let mut lexer = Lexer::new();

        let first = lexer.tokenize("1 + ans").unwrap();
        let second = lexer.tokenize("ans * 2").unwrap();

        assert_eq!(first[2].kind, TokenKind::Sym("ans".into()));
        assert_eq!(second[0].kind, TokenKind::Sym("ans".into()));
    }
}
