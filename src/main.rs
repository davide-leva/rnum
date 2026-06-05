use calc::{
    eval::{EvalError, EvalOutput, Evaluator},
    lexer::LexingError,
    parser::ParseError,
    shell::{Command, CommandResult, FnCommand, Shell, VarCommand},
};

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const UNDERLINE: &str = "\x1b[4m";

fn main() {
    let mut evaluator = Evaluator::new();
    let mut shell = Shell::new();

    if let Err(err) = shell.listen(|command| match command {
        Command::Eval(expr) => cmd_eval(&mut evaluator, &expr),
        Command::Ast(expr) => cmd_ast(&mut evaluator, &expr),
        Command::Var(command) => cmd_var(&mut evaluator, command),
        Command::Fn(command) => cmd_fn(&mut evaluator, command),
        Command::Save(filename) => cmd_save(&evaluator, filename),
        Command::Help => CommandResult::success(print_help()),
        Command::Exit => CommandResult::exit(),
    }) {
        eprintln!("Input error: {err}");
    }
}

fn cmd_eval(evaluator: &mut Evaluator, expr: &str) -> CommandResult {
    match evaluator.eval_line(expr) {
        Ok(EvalOutput::Value(result)) => CommandResult::success(result.to_string()),
        Ok(EvalOutput::FunctionDefined { name, params }) => {
            CommandResult::success(format!("defined {}({})", name, params.join(", ")))
        }
        Err(err) => CommandResult::error(format_error(expr, &err)),
    }
}

fn cmd_ast(evaluator: &mut Evaluator, expr: &str) -> CommandResult {
    if expr.is_empty() {
        return CommandResult::error("Usage: :ast <expr>".to_string());
    }

    match evaluator.ast(expr) {
        Ok(ast) => CommandResult::success(ast.to_string()),
        Err(err) => CommandResult::error(format_error(expr, &err)),
    }
}

fn print_help() -> String {
    [
        "Commands:",
        "  :help              show this help",
        "  :ast <expr>        print the parsed AST",
        "  :var               list variables",
        "  :var del <name>    delete a variable",
        "  :fn                list user functions",
        "  :fn del <name>     delete a user function",
        "  :save <filename>   save history, variables, and functions",
        "  :exit, :quit       exit",
        "",
        "Operators:",
        "  =  +  -  *  /  %  ^",
        "",
        "Functions:",
        "  sqrt(x)  sin(x)  cos(x)  tan(x)  ln(x)  log(x)  abs(x)",
        "  f(a) = a + 1",
        "  f(a, b) = a * b + sin(a)",
    ]
    .join("\n")
}

fn format_error(input: &str, error: &EvalError) -> String {
    let diagnostic = Diagnostic::from_eval_error(input, error);

    format!(
        "{BOLD}{RED}error:{RESET} {}\n  {}\n  {}{BOLD}{RED}^{RESET} {}",
        diagnostic.message,
        underline_input(input, diagnostic.pos, diagnostic.len),
        " ".repeat(column(input, diagnostic.pos)),
        diagnostic.detail
    )
}

fn cmd_var(evaluator: &mut Evaluator, command: VarCommand) -> CommandResult {
    match command {
        VarCommand::List => CommandResult::success(format_vars(evaluator)),
        VarCommand::Del(name) => {
            if name.is_empty() {
                CommandResult::error("Usage: :var del <name>".to_string())
            } else if evaluator.del_var(&name) {
                CommandResult::success(format!("Deleted variable `{name}`"))
            } else {
                CommandResult::error(format!("Variable `{name}` not found"))
            }
        }
    }
}

fn cmd_fn(evaluator: &mut Evaluator, command: FnCommand) -> CommandResult {
    match command {
        FnCommand::List => CommandResult::success(format_functions(evaluator)),
        FnCommand::Del(name) => {
            if name.is_empty() {
                CommandResult::error("Usage: :fn del <name>".to_string())
            } else if evaluator.del_function(&name) {
                CommandResult::success(format!("Deleted function `{name}`"))
            } else {
                CommandResult::error(format!("Function `{name}` not found"))
            }
        }
    }
}

fn cmd_save(evaluator: &Evaluator, filename: String) -> CommandResult {
    if filename.is_empty() {
        return CommandResult::error("Usage: :save <filename>".to_string());
    }

    CommandResult::save(filename, format_save_state(evaluator))
}

fn format_vars(evaluator: &Evaluator) -> String {
    let vars = evaluator.vars();

    if vars.is_empty() {
        return "No variables defined".to_string();
    }

    vars.into_iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_functions(evaluator: &Evaluator) -> String {
    let functions = evaluator.functions();

    if functions.is_empty() {
        return "No user functions defined".to_string();
    }

    functions
        .into_iter()
        .map(|(name, params, body)| format!("{}({}) = {}", name, params.join(", "), body))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_save_state(evaluator: &Evaluator) -> String {
    let mut output = String::new();

    output.push_str("[variables]\n");
    for (name, value) in evaluator.vars() {
        output.push_str(&format!("{name} = {value}\n"));
    }

    output.push_str("\n[functions]\n");
    for (name, params, body) in evaluator.functions() {
        output.push_str(&format!("{}({}) = {}\n", name, params.join(", "), body));
    }

    output
}

struct Diagnostic {
    pos: usize,
    len: usize,
    message: String,
    detail: String,
}

impl Diagnostic {
    fn from_eval_error(input: &str, error: &EvalError) -> Self {
        match error {
            EvalError::LexingError(error) => Self::from_lexing_error(error),
            EvalError::ParseError(error) => Self::from_parse_error(input, error),
            EvalError::DivisionByZero => {
                let pos = find_zero_divisor(input).unwrap_or(input.len());
                Self {
                    pos,
                    len: 1,
                    message: "division by zero".to_string(),
                    detail: "the divisor evaluates to zero".to_string(),
                }
            }
            EvalError::UnknownVariable(name) => {
                let pos = input.find(name).unwrap_or(0);
                Self {
                    pos,
                    len: name.len().max(1),
                    message: format!("unknown variable `{name}`"),
                    detail: "this variable has not been assigned yet".to_string(),
                }
            }
            EvalError::UnknownFunction(name) => {
                let pos = input.find(name).unwrap_or(0);
                Self {
                    pos,
                    len: name.len().max(1),
                    message: format!("unknown function `{name}`"),
                    detail: "this function is not defined".to_string(),
                }
            }
            EvalError::InvalidArgument { function, value } => {
                let (pos, len) = function_argument_span(input, function).unwrap_or((0, 1));
                Self {
                    pos,
                    len,
                    message: format!("invalid argument for `{function}`"),
                    detail: format!("`{function}` cannot accept {value} here"),
                }
            }
            EvalError::WrongArgCount {
                function,
                expected,
                actual,
            } => {
                let pos = input.find(function).unwrap_or(0);
                Self {
                    pos,
                    len: function.len().max(1),
                    message: format!("wrong number of arguments for `{function}`"),
                    detail: format!("expected {expected}, got {actual}"),
                }
            }
            EvalError::RecursiveFunctionCall { function } => {
                let pos = input.find(function).unwrap_or(0);
                Self {
                    pos,
                    len: function.len().max(1),
                    message: format!("recursive function call `{function}`"),
                    detail: "recursive user functions are not supported".to_string(),
                }
            }
        }
    }

    fn from_lexing_error(error: &LexingError) -> Self {
        match error {
            LexingError::UnexpectedCharacter { ch, pos } => Self {
                pos: *pos,
                len: ch.len_utf8(),
                message: format!("unexpected character `{ch}`"),
                detail: "this character is not part of the expression language".to_string(),
            },
            LexingError::InvalidNumber { text, pos } => Self {
                pos: *pos,
                len: text.len().max(1),
                message: format!("invalid number `{text}`"),
                detail: "this number literal cannot be parsed".to_string(),
            },
        }
    }

    fn from_parse_error(input: &str, error: &ParseError) -> Self {
        match error {
            ParseError::UnexpectedToken { pos, found } => Self {
                pos: *pos,
                len: token_len_at(input, *pos),
                message: format!("unexpected token {found}"),
                detail: "the parser cannot use this token here".to_string(),
            },
            ParseError::UnexpectedEOE { pos } => Self {
                pos: *pos,
                len: 1,
                message: "unexpected end of expression".to_string(),
                detail: "the parser expected another expression here".to_string(),
            },
            ParseError::InvalidAssign { pos } => {
                let (pos, len) = invalid_assignment_span(input, *pos);

                Self {
                    pos,
                    len,
                    message: "invalid assignment target".to_string(),
                    detail: "the left side of `=` must be a variable name".to_string(),
                }
            }
        }
    }
}

fn invalid_assignment_span(input: &str, assign_pos: usize) -> (usize, usize) {
    let left = &input[..assign_pos.min(input.len())];
    let start = left
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(pos, _)| pos)
        .unwrap_or(assign_pos);
    let end = left
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(pos, ch)| pos + ch.len_utf8())
        .unwrap_or(assign_pos);

    if start < end {
        (start, end - start)
    } else {
        let assign_pos = assign_pos.min(input.len());
        (assign_pos, token_len_at(input, assign_pos))
    }
}

fn underline_input(input: &str, pos: usize, len: usize) -> String {
    let pos = pos.min(input.len());

    if pos >= input.len() {
        return format!("{input}{UNDERLINE}{RED} {RESET}");
    }

    let end = next_boundary(input, pos + len.max(1));
    let (prefix, rest) = input.split_at(pos);
    let (problem, suffix) = rest.split_at(end - pos);

    format!("{prefix}{UNDERLINE}{RED}{problem}{RESET}{suffix}")
}

fn next_boundary(input: &str, mut pos: usize) -> usize {
    pos = pos.min(input.len());

    while pos < input.len() && !input.is_char_boundary(pos) {
        pos += 1;
    }

    pos
}

fn column(input: &str, pos: usize) -> usize {
    input[..pos.min(input.len())].chars().count()
}

fn find_zero_divisor(input: &str) -> Option<usize> {
    for op in ["/", "%"] {
        if let Some(op_pos) = input.find(op) {
            let after_op = op_pos + op.len();
            let spaces = input[after_op..]
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .map(char::len_utf8)
                .sum::<usize>();
            let candidate = after_op + spaces;

            if input[candidate..].starts_with('0') {
                return Some(candidate);
            }
        }
    }

    None
}

fn token_len_at(input: &str, pos: usize) -> usize {
    if pos >= input.len() {
        return 1;
    }

    input[pos..]
        .chars()
        .take_while(|ch| !ch.is_whitespace() && !is_operator(*ch))
        .map(char::len_utf8)
        .sum::<usize>()
        .max(1)
}

fn is_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '^' | '=' | '(' | ')')
}

fn function_argument_span(input: &str, function: &str) -> Option<(usize, usize)> {
    let start = input.find(function)?;
    let open = input[start..].find('(')? + start;
    let after_open = open + 1;
    let spaces = input[after_open..]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();

    let arg_start = after_open + spaces;
    let close = input[arg_start..]
        .rfind(')')
        .map(|pos| arg_start + pos)
        .unwrap_or(input.len());
    let arg_end = input[..close]
        .char_indices()
        .rev()
        .find(|(pos, ch)| *pos >= arg_start && !ch.is_whitespace())
        .map(|(pos, ch)| pos + ch.len_utf8())
        .unwrap_or(arg_start);

    Some((arg_start, (arg_end - arg_start).max(1)))
}
