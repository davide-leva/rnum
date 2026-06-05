use std::collections::HashMap;

use crate::{
    lexer::{Lexer, LexingError},
    parser::{BinaryOp, Expr, ParseError, Parser, UnaryOp},
    symbols::SymbolCache,
};

#[derive(Debug)]
pub enum EvalError {
    LexingError(LexingError),
    ParseError(ParseError),
    DivisionByZero,
    UnknownVariable(String),
    UnknownFunction(String),
    InvalidArgument {
        function: String,
        value: f64,
    },
    WrongArgCount {
        function: String,
        expected: usize,
        actual: usize,
    },
    RecursiveFunctionCall {
        function: String,
    },
}

pub enum EvalOutput {
    Value(f64),
    FunctionDefined { name: String, params: Vec<String> },
}

#[derive(Clone)]
struct Function {
    params: Vec<String>,
    body: Expr,
}

#[derive(Default)]
pub struct Evaluator {
    symbols: SymbolCache,
    functions: HashMap<String, Function>,
    scopes: Vec<HashMap<String, f64>>,
    call_stack: Vec<String>,
    parser: Parser,
    lexer: Lexer,
    ans: f64,
}

impl Evaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn eval(&mut self, input: &str) -> Result<f64, EvalError> {
        match self.eval_line(input)? {
            EvalOutput::Value(value) => Ok(value),
            EvalOutput::FunctionDefined { .. } => Ok(0.0),
        }
    }

    pub fn eval_line(&mut self, input: &str) -> Result<EvalOutput, EvalError> {
        let tokens = self
            .lexer
            .tokenize(&input)
            .map_err(|le| EvalError::LexingError(le))?;

        let expr = self
            .parser
            .parse(tokens)
            .map_err(|pe| EvalError::ParseError(pe))?;

        if let Expr::FunctionDef { name, params, .. } = &expr {
            self.eval_expr(&expr)?;
            return Ok(EvalOutput::FunctionDefined {
                name: name.clone(),
                params: params.clone(),
            });
        }

        self.eval_and_set(&expr).map(EvalOutput::Value)
    }

    pub fn ast(&mut self, input: &str) -> Result<Expr, EvalError> {
        let tokens = self
            .lexer
            .tokenize(&input)
            .map_err(|le| EvalError::LexingError(le))?;

        self.parser
            .parse(tokens)
            .map_err(|pe| EvalError::ParseError(pe))
    }

    pub fn vars(&self) -> Vec<(&str, f64)> {
        self.symbols.entries()
    }

    pub fn del_var(&mut self, name: &str) -> bool {
        self.symbols.del(name)
    }

    pub fn functions(&self) -> Vec<(String, Vec<String>, String)> {
        let mut functions = self
            .functions
            .iter()
            .map(|(name, function)| {
                (
                    name.clone(),
                    function.params.clone(),
                    function.body.to_string(),
                )
            })
            .collect::<Vec<_>>();

        functions.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));
        functions
    }

    pub fn del_function(&mut self, name: &str) -> bool {
        self.functions.remove(name).is_some()
    }

    fn eval_and_set(&mut self, expr: &Expr) -> Result<f64, EvalError> {
        self.ans = self.eval_expr(&expr)?;
        self.symbols.save("ans".to_string(), self.ans);
        Ok(self.ans)
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<f64, EvalError> {
        match expr {
            Expr::Number(val) => Ok(*val),
            Expr::Symbol(name) => self.resolve_symbol(name),
            Expr::Assign { name, expr } => {
                let value = self.eval_expr(expr)?;
                self.symbols.save(name.clone(), value);
                Ok(value)
            }
            Expr::FunctionDef { name, params, body } => {
                self.functions.insert(
                    name.clone(),
                    Function {
                        params: params.clone(),
                        body: body.as_ref().clone(),
                    },
                );

                Ok(0.0)
            }
            Expr::Call { name, args } => {
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval_expr(arg)?);
                }

                self.call_function(name, values)
            }
            Expr::Unary { op, expr: inner } => {
                let value = self.eval_expr(inner)?;

                match op {
                    UnaryOp::Neg => Ok(-value),
                }
            }
            Expr::Binary { left, op, right } => {
                let left = self.eval_expr(left)?;
                let right = self.eval_expr(right)?;

                match op {
                    BinaryOp::Add => Ok(left + right),
                    BinaryOp::Sub => Ok(left - right),
                    BinaryOp::Mul => Ok(left * right),
                    BinaryOp::Div => {
                        if right == 0.0 {
                            Err(EvalError::DivisionByZero)
                        } else {
                            Ok(left / right)
                        }
                    }
                    BinaryOp::Pow => Ok(left.powf(right)),
                    BinaryOp::Mod => {
                        if right == 0.0 {
                            Err(EvalError::DivisionByZero)
                        } else {
                            Ok(left % right)
                        }
                    }
                }
            }
        }
    }

    fn resolve_symbol(&self, name: &str) -> Result<f64, EvalError> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Ok(*value);
            }
        }

        self.symbols
            .get(name)
            .ok_or(EvalError::UnknownVariable(name.to_string()))
    }

    fn call_function(&mut self, name: &str, values: Vec<f64>) -> Result<f64, EvalError> {
        if is_builtin(name) {
            return self.call_builtin(name, values);
        }

        let Some(function) = self.functions.get(name).cloned() else {
            return Err(EvalError::UnknownFunction(name.to_string()));
        };

        if self.call_stack.iter().any(|function| function == name) {
            return Err(EvalError::RecursiveFunctionCall {
                function: name.to_string(),
            });
        }

        if function.params.len() != values.len() {
            return Err(EvalError::WrongArgCount {
                function: name.to_string(),
                expected: function.params.len(),
                actual: values.len(),
            });
        }

        let scope = function
            .params
            .iter()
            .cloned()
            .zip(values)
            .collect::<HashMap<_, _>>();

        self.scopes.push(scope);
        self.call_stack.push(name.to_string());
        let result = self.eval_expr(&function.body);
        self.call_stack.pop();
        self.scopes.pop();

        result
    }

    fn call_builtin(&self, name: &str, values: Vec<f64>) -> Result<f64, EvalError> {
        if values.len() != 1 {
            return Err(EvalError::WrongArgCount {
                function: name.to_string(),
                expected: 1,
                actual: values.len(),
            });
        }

        let value = values[0];

        match name {
            "sqrt" => {
                if value < 0.0 {
                    Err(EvalError::InvalidArgument {
                        function: name.to_string(),
                        value,
                    })
                } else {
                    Ok(value.sqrt())
                }
            }
            "sin" => Ok(value.sin()),
            "cos" => Ok(value.cos()),
            "tan" => Ok(value.tan()),
            "ln" => {
                if value <= 0.0 {
                    Err(EvalError::InvalidArgument {
                        function: name.to_string(),
                        value,
                    })
                } else {
                    Ok(value.ln())
                }
            }
            "log" => {
                if value <= 0.0 {
                    Err(EvalError::InvalidArgument {
                        function: name.to_string(),
                        value,
                    })
                } else {
                    Ok(value.log10())
                }
            }
            "abs" => Ok(value.abs()),
            _ => unreachable!("builtin list and dispatch are out of sync"),
        }
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(name, "sqrt" | "sin" | "cos" | "tan" | "ln" | "log" | "abs")
}

#[cfg(test)]
mod tests {
    use super::{EvalError, Evaluator};

    #[test]
    fn assigns_and_reads_variable() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("x = 10").unwrap(), 10.0);
        assert_eq!(evaluator.eval("x + 2").unwrap(), 12.0);
    }

    #[test]
    fn assignment_is_right_associative() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("a = b = 3").unwrap(), 3.0);
        assert_eq!(evaluator.eval("a + b").unwrap(), 6.0);
    }

    #[test]
    fn evaluates_power_and_modulo() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("2 ^ 3 ^ 2").unwrap(), 512.0);
        assert_eq!(evaluator.eval("10 % 4").unwrap(), 2.0);
    }

    #[test]
    fn evaluates_math_functions() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("sqrt(9)").unwrap(), 3.0);
        assert_eq!(evaluator.eval("abs(-4)").unwrap(), 4.0);
        assert_eq!(evaluator.eval("log(100)").unwrap(), 2.0);
    }

    #[test]
    fn defines_and_calls_user_function() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("f(a) = a + 1").unwrap(), 0.0);
        assert_eq!(evaluator.eval("f(10)").unwrap(), 11.0);
    }

    #[test]
    fn user_function_accepts_multiple_args() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("f(a, b) = a * b + sin(a)").unwrap(), 0.0);
        assert_eq!(evaluator.eval("f(2, 3)").unwrap(), 6.0 + 2.0_f64.sin());
    }

    #[test]
    fn user_function_can_call_user_function() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("g(x) = x * 2").unwrap(), 0.0);
        assert_eq!(evaluator.eval("f(a, b) = a + g(b)").unwrap(), 0.0);
        assert_eq!(evaluator.eval("f(3, 4)").unwrap(), 11.0);
    }

    #[test]
    fn rejects_direct_recursive_user_function() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("f(x) = f(x)").unwrap(), 0.0);

        let err = evaluator.eval("f(2)").unwrap_err();

        assert!(matches!(
            err,
            EvalError::RecursiveFunctionCall { function } if function == "f"
        ));
    }

    #[test]
    fn rejects_indirect_recursive_user_function() {
        let mut evaluator = Evaluator::new();

        assert_eq!(evaluator.eval("f(x) = g(x)").unwrap(), 0.0);
        assert_eq!(evaluator.eval("g(x) = f(x)").unwrap(), 0.0);

        let err = evaluator.eval("f(2)").unwrap_err();

        assert!(matches!(
            err,
            EvalError::RecursiveFunctionCall { function } if function == "f"
        ));
    }

    #[test]
    fn gets_ast_without_evaluating_expression() {
        let mut evaluator = Evaluator::new();

        let ast = evaluator.ast("x = 1 + 2 * 3").unwrap();

        assert_eq!(ast.to_string(), "Assign(x, Add(1, Mul(2, 3)))");
        assert!(evaluator.eval("x").is_err());
    }
}
