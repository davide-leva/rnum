use std::collections::HashMap;

use crate::{
    lexer::{Lexer, LexingError, NumberFormat, UnitFormat},
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
    ReservedCommandName(String),
}

pub enum EvalOutput {
    Value(EvalValue),
    FunctionDefined { name: String, params: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvalValue {
    pub value: f64,
    pub format: Option<NumberFormat>,
}

impl EvalValue {
    fn plain(value: f64) -> Self {
        Self {
            value,
            format: None,
        }
    }

    fn new(value: f64, format: Option<NumberFormat>) -> Self {
        Self { value, format }
    }

    fn with_format(self, format: Option<NumberFormat>) -> Self {
        Self {
            value: self.value,
            format,
        }
    }

    fn combine_format(self, other: Self) -> Option<NumberFormat> {
        combine_formats(self.format, other.format)
    }

    pub fn formatted(&self) -> String {
        match self.format {
            Some(NumberFormat::Unit(format)) => format_unit(self.value, format),
            Some(NumberFormat::Scientific) => format_scientific(self.value),
            None => self.value.to_string(),
        }
    }
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
            EvalOutput::Value(value) => Ok(value.value),
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

    pub fn reset(&mut self) {
        self.symbols.clear();
        self.functions.clear();
        self.scopes.clear();
        self.call_stack.clear();
        self.ans = 0.0;
    }

    fn eval_and_set(&mut self, expr: &Expr) -> Result<EvalValue, EvalError> {
        let value = self.eval_expr(expr)?;
        self.ans = value.value;
        self.symbols.save("ans".to_string(), self.ans);
        Ok(value)
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<EvalValue, EvalError> {
        match expr {
            Expr::Number { value, format } => Ok(EvalValue::new(*value, *format)),
            Expr::Symbol(name) => self.resolve_symbol(name),
            Expr::Assign { name, expr } => {
                if is_reserved_command_name(name) {
                    return Err(EvalError::ReservedCommandName(name.clone()));
                }

                let value = self.eval_expr(expr)?;
                self.symbols.save(name.clone(), value.value);
                Ok(value)
            }
            Expr::FunctionDef { name, params, body } => {
                if is_reserved_command_name(name) {
                    return Err(EvalError::ReservedCommandName(name.clone()));
                }

                self.functions.insert(
                    name.clone(),
                    Function {
                        params: params.clone(),
                        body: body.as_ref().clone(),
                    },
                );

                Ok(EvalValue::plain(0.0))
            }
            Expr::Call { name, args } => {
                let mut values = Vec::new();
                let mut format = None;
                for arg in args {
                    let value = self.eval_expr(arg)?;
                    format = combine_formats(format, value.format);
                    values.push(value.value);
                }

                let result = self.call_function(name, values)?;
                Ok(result.with_format(combine_formats(format, result.format)))
            }
            Expr::Unary { op, expr: inner } => {
                let value = self.eval_expr(inner)?;

                match op {
                    UnaryOp::Neg => Ok(EvalValue::new(-value.value, value.format)),
                }
            }
            Expr::Binary { left, op, right } => {
                let left = self.eval_expr(left)?;
                let right = self.eval_expr(right)?;
                let format = left.combine_format(right);

                match op {
                    BinaryOp::Add => Ok(EvalValue::new(left.value + right.value, format)),
                    BinaryOp::Sub => Ok(EvalValue::new(left.value - right.value, format)),
                    BinaryOp::Mul => Ok(EvalValue::new(left.value * right.value, format)),
                    BinaryOp::Div => {
                        if right.value == 0.0 {
                            Err(EvalError::DivisionByZero)
                        } else {
                            Ok(EvalValue::new(left.value / right.value, format))
                        }
                    }
                    BinaryOp::Pow => Ok(EvalValue::new(left.value.powf(right.value), format)),
                    BinaryOp::Mod => {
                        if right.value == 0.0 {
                            Err(EvalError::DivisionByZero)
                        } else {
                            Ok(EvalValue::new(left.value % right.value, format))
                        }
                    }
                }
            }
        }
    }

    fn resolve_symbol(&self, name: &str) -> Result<EvalValue, EvalError> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Ok(EvalValue::plain(*value));
            }
        }

        self.symbols
            .get(name)
            .map(EvalValue::plain)
            .ok_or(EvalError::UnknownVariable(name.to_string()))
    }

    fn call_function(&mut self, name: &str, values: Vec<f64>) -> Result<EvalValue, EvalError> {
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

    fn call_builtin(&self, name: &str, values: Vec<f64>) -> Result<EvalValue, EvalError> {
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
                    Ok(EvalValue::plain(value.sqrt()))
                }
            }
            "sin" => Ok(EvalValue::plain(value.sin())),
            "cos" => Ok(EvalValue::plain(value.cos())),
            "tan" => Ok(EvalValue::plain(value.tan())),
            "ln" => {
                if value <= 0.0 {
                    Err(EvalError::InvalidArgument {
                        function: name.to_string(),
                        value,
                    })
                } else {
                    Ok(EvalValue::plain(value.ln()))
                }
            }
            "log" => {
                if value <= 0.0 {
                    Err(EvalError::InvalidArgument {
                        function: name.to_string(),
                        value,
                    })
                } else {
                    Ok(EvalValue::plain(value.log10()))
                }
            }
            "abs" => Ok(EvalValue::plain(value.abs())),
            _ => unreachable!("builtin list and dispatch are out of sync"),
        }
    }
}

fn combine_formats(
    left: Option<NumberFormat>,
    right: Option<NumberFormat>,
) -> Option<NumberFormat> {
    match (left, right) {
        (Some(NumberFormat::Unit(_)), _) => left,
        (_, Some(NumberFormat::Unit(_))) => right,
        (Some(NumberFormat::Scientific), _) | (_, Some(NumberFormat::Scientific)) => {
            Some(NumberFormat::Scientific)
        }
        (None, None) => None,
    }
}

fn format_scientific(value: f64) -> String {
    if value == 0.0 {
        return "0e0".to_string();
    }

    let text = format!("{value:e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };

    format!(
        "{}e{}",
        trim_float(mantissa),
        exponent.trim_start_matches('+')
    )
}

fn format_unit(value: f64, format: UnitFormat) -> String {
    let suffixes = match format {
        UnitFormat::LowerB => ["b", "Kb", "Mb", "Gb", "Tb", "Pb", "Eb"],
        UnitFormat::UpperB => ["B", "KB", "MB", "GB", "TB", "PB", "EB"],
    };

    let mut scaled = value;
    let mut suffix = suffixes[0];

    for candidate in suffixes.iter().skip(1) {
        if scaled.abs() < 1024.0 {
            break;
        }

        scaled /= 1024.0;
        suffix = candidate;
    }

    format!("{}{}", trim_float(&format!("{scaled:.12}")), suffix)
}

fn trim_float(text: &str) -> String {
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(name, "sqrt" | "sin" | "cos" | "tan" | "ln" | "log" | "abs")
}

fn is_reserved_command_name(name: &str) -> bool {
    matches!(
        name,
        "help" | "ast" | "var" | "fn" | "save" | "clear" | "reset" | "exit" | "quit"
    )
}

#[cfg(test)]
mod tests {
    use super::{EvalError, EvalOutput, Evaluator};

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
        assert_eq!(evaluator.eval("2 ** 3 ** 2").unwrap(), 512.0);
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

    #[test]
    fn formats_scientific_result_when_expression_uses_scientific_literal() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("10e5 + 1").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 1_000_001.0);
        assert_eq!(result.formatted(), "1.000001e6");
    }

    #[test]
    fn formats_unit_result_when_expression_uses_lower_b_literal() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("1Kb + 512").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 1536.0);
        assert_eq!(result.formatted(), "1.5Kb");
    }

    #[test]
    fn formats_unit_result_when_expression_uses_upper_b_literal() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("1KB + 512B").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 1536.0);
        assert_eq!(result.formatted(), "1.5KB");
    }

    #[test]
    fn unit_format_takes_precedence_over_scientific_result() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("1KB + 1024e1").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 11_264.0);
        assert_eq!(result.formatted(), "11KB");
    }

    #[test]
    fn formats_unit_result_with_binary_base() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("1b * 1024").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 1024.0);
        assert_eq!(result.formatted(), "1Kb");
    }

    #[test]
    fn formats_upper_unit_result_with_binary_base() {
        let mut evaluator = Evaluator::new();

        let EvalOutput::Value(result) = evaluator.eval_line("2KB * 1024^2").unwrap() else {
            panic!("expected value");
        };

        assert_eq!(result.value, 2_147_483_648.0);
        assert_eq!(result.formatted(), "2GB");
    }

    #[test]
    fn rejects_assignment_to_command_name() {
        let mut evaluator = Evaluator::new();

        let err = evaluator.eval("help = 1").unwrap_err();

        assert!(matches!(err, EvalError::ReservedCommandName(name) if name == "help"));
    }

    #[test]
    fn rejects_function_definition_with_command_name() {
        let mut evaluator = Evaluator::new();

        let err = evaluator.eval("reset(x) = x").unwrap_err();

        assert!(matches!(err, EvalError::ReservedCommandName(name) if name == "reset"));
    }

    #[test]
    fn reset_clears_variables_and_functions() {
        let mut evaluator = Evaluator::new();

        evaluator.eval("x = 10").unwrap();
        evaluator.eval("f(a) = a + 1").unwrap();
        evaluator.reset();

        assert!(matches!(
            evaluator.eval("x"),
            Err(EvalError::UnknownVariable(_))
        ));
        assert!(matches!(
            evaluator.eval("f(1)"),
            Err(EvalError::UnknownFunction(_))
        ));
    }
}
