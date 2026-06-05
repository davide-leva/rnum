use std::fmt::Display;

use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Symbol(String),

    Assign {
        name: String,
        expr: Box<Expr>,
    },

    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Box<Expr>,
    },

    Call {
        name: String,
        args: Vec<Expr>,
    },

    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },

    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
}

impl Expr {
    pub fn num(num: f64) -> Self {
        return Self::Number(num);
    }

    pub fn sym(sym: String) -> Self {
        return Self::Symbol(sym);
    }

    pub fn assign(name: String, expr: Expr) -> Self {
        return Self::Assign {
            name,
            expr: Box::new(expr),
        };
    }

    pub fn function_def(name: String, params: Vec<String>, body: Expr) -> Self {
        return Self::FunctionDef {
            name,
            params,
            body: Box::new(body),
        };
    }

    pub fn call(name: String, args: Vec<Expr>) -> Self {
        return Self::Call { name, args };
    }

    pub fn unary(op: UnaryOp, expr: Expr) -> Self {
        return Self::Unary {
            op,
            expr: Box::new(expr),
        };
    }

    pub fn binary(left: Expr, op: BinaryOp, right: Expr) -> Self {
        return Self::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Number(num) => {
                write!(f, "{}", num)
            }
            Expr::Symbol(sym) => {
                write!(f, "{}", sym)
            }
            Expr::Assign { name, expr } => {
                write!(f, "Assign({}, {})", name, expr)
            }
            Expr::FunctionDef { name, params, body } => {
                write!(
                    f,
                    "FunctionDef({}, [{}], {})",
                    name,
                    params.join(", "),
                    body
                )
            }
            Expr::Call { name, args } => {
                write!(f, "Call({}, [", name)?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, "])")
            }
            Expr::Unary { op, expr } => {
                write!(f, "{:?}({})", op, expr)
            }
            Expr::Binary { left, op, right } => {
                write!(f, "{:?}({}, {})", op, left, right)
            }
        }
    }
}

#[derive(Default)]
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken { pos: usize, found: String },
    UnexpectedEOE { pos: usize },
    InvalidAssign { pos: usize },
}

impl Parser {
    pub fn new() -> Self {
        Parser::default()
    }

    pub fn parsed_token(&self) -> usize {
        self.current
    }

    pub fn parse(&mut self, tokens: Vec<Token>) -> Result<Expr, ParseError> {
        self.tokens = tokens;
        self.current = 0;

        let expr = self.parse_assignment()?;

        match self.peek_kind() {
            TokenKind::EOE => Ok(expr),
            _ => Err(ParseError::UnexpectedToken {
                pos: self.current_pos(),
                found: self.peek_kind().name().to_string(),
            }),
        }
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.current].kind
    }

    fn current_pos(&self) -> usize {
        self.tokens[self.current].pos
    }

    fn advance(&mut self) {
        self.current += 1;
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_expr()?;

        if !matches!(self.peek_kind(), TokenKind::Eqa) {
            return Ok(left);
        }

        let assign_pos = self.current_pos();
        self.advance();

        let right = self.parse_assignment()?;

        match left {
            Expr::Symbol(name) => Ok(Expr::assign(name, right)),
            Expr::Call { name, args } => {
                let mut params = Vec::new();

                for arg in args {
                    match arg {
                        Expr::Symbol(name) => params.push(name),
                        _ => {
                            return Err(ParseError::InvalidAssign { pos: assign_pos });
                        }
                    }
                }

                Ok(Expr::function_def(name, params, right))
            }
            _ => {
                return Err(ParseError::InvalidAssign { pos: assign_pos });
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;

        loop {
            let op = match self.peek_kind() {
                TokenKind::Plu => BinaryOp::Add,
                TokenKind::Min => BinaryOp::Sub,
                _ => break,
            };

            self.advance();

            let right = self.parse_term()?;

            left = Expr::binary(left, op, right);
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_power()?;

        loop {
            let op = match self.peek_kind() {
                TokenKind::Str => BinaryOp::Mul,
                TokenKind::Sle => BinaryOp::Div,
                TokenKind::Per => BinaryOp::Mod,
                _ => break,
            };

            self.advance();

            let right = self.parse_power()?;

            left = Expr::binary(left, op, right)
        }

        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_unary()?;

        if !matches!(self.peek_kind(), TokenKind::Hat) {
            return Ok(left);
        }

        self.advance();

        let right = self.parse_power()?;
        Ok(Expr::binary(left, BinaryOp::Pow, right))
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek_kind() {
            TokenKind::Min => {
                self.advance();

                Ok(Expr::unary(UnaryOp::Neg, self.parse_unary()?))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek_kind() {
            TokenKind::Num(value) => {
                let value = *value;
                self.advance();
                Ok(Expr::num(value))
            }
            TokenKind::Sym(symbol) => {
                let value = symbol.clone();
                self.advance();

                if !matches!(self.peek_kind(), TokenKind::LPa) {
                    return Ok(Expr::sym(value));
                }

                let args = self.parse_call_args()?;
                Ok(Expr::call(value, args))
            }
            TokenKind::LPa => {
                self.advance();

                let expr = self.parse_expr()?;

                match self.peek_kind() {
                    TokenKind::RPa => {
                        self.advance();
                        Ok(expr)
                    }
                    _ => Err(ParseError::UnexpectedToken {
                        pos: self.current_pos(),
                        found: self.peek_kind().name().to_string(),
                    }),
                }
            }
            TokenKind::EOE => Err(ParseError::UnexpectedEOE {
                pos: self.current_pos(),
            }),
            _ => Err(ParseError::UnexpectedToken {
                pos: self.current_pos(),
                found: self.peek_kind().name().to_string(),
            }),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.advance();

        if matches!(self.peek_kind(), TokenKind::RPa) {
            self.advance();
            return Ok(Vec::new());
        }

        let mut args = Vec::new();

        loop {
            args.push(self.parse_assignment()?);

            match self.peek_kind() {
                TokenKind::Com => self.advance(),
                TokenKind::RPa => {
                    self.advance();
                    return Ok(args);
                }
                _ => {
                    return Err(ParseError::UnexpectedToken {
                        pos: self.current_pos(),
                        found: self.peek_kind().name().to_string(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn parses_expression_from_tokens() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("1 + 2 * 3").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Add(1, Mul(2, 3))");
    }

    #[test]
    fn reuses_parser_state_between_token_streams() {
        let mut lexer = Lexer::new();
        let mut parser = Parser::new();

        let first = parser.parse(lexer.tokenize("10").unwrap()).unwrap();
        let second = parser.parse(lexer.tokenize("-2").unwrap()).unwrap();

        assert_eq!(first.to_string(), "10");
        assert_eq!(second.to_string(), "Neg(2)");
    }

    #[test]
    fn parses_expression_with_symbol() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("1 + 2 * ans").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Add(1, Mul(2, ans))");
    }

    #[test]
    fn parses_assignment() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("x = 1 + 2 * 3").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Assign(x, Add(1, Mul(2, 3)))");
    }

    #[test]
    fn rejects_assignment_to_non_symbol() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("1 = 2").unwrap();
        let mut parser = Parser::new();

        let err = parser.parse(tokens).unwrap_err();

        assert!(matches!(err, ParseError::InvalidAssign { .. }));
    }

    #[test]
    fn parses_power_with_higher_precedence_than_term() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("2 * 3 ^ 4").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Mul(2, Pow(3, 4))");
    }

    #[test]
    fn parses_power_as_right_associative() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("2 ^ 3 ^ 4").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Pow(2, Pow(3, 4))");
    }

    #[test]
    fn parses_function_call() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("sqrt(1 + 8)").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(expr.to_string(), "Call(sqrt, [Add(1, 8)])");
    }

    #[test]
    fn parses_function_definition() {
        let mut lexer = Lexer::new();
        let tokens = lexer.tokenize("f(a, b) = a * b + sin(a)").unwrap();
        let mut parser = Parser::new();

        let expr = parser.parse(tokens).unwrap();

        assert_eq!(
            expr.to_string(),
            "FunctionDef(f, [a, b], Add(Mul(a, b), Call(sin, [a])))"
        );
    }
}
