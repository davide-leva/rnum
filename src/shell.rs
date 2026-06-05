use std::{
    fs,
    io::{self, Read, Write},
    process::{Command as ProcessCommand, Stdio},
};

const PROMPT: &str = "rnum >>> ";
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const ORANGE: &str = "\x1b[38;5;208m";
const RED: &str = "\x1b[31m";

pub enum Command {
    Eval(String),
    Ast(String),
    Var(VarCommand),
    Fn(FnCommand),
    Save(String),
    Help,
    Exit,
}

pub enum VarCommand {
    List,
    Del(String),
}

pub enum FnCommand {
    List,
    Del(String),
}

pub enum CommandResult {
    Success(String),
    Error(String),
    Save { filename: String, content: String },
    Exit,
}

impl CommandResult {
    pub fn success(output: String) -> Self {
        Self::Success(output)
    }

    pub fn error(output: String) -> Self {
        Self::Error(output)
    }

    pub fn exit() -> Self {
        Self::Exit
    }

    pub fn save(filename: String, content: String) -> Self {
        Self::Save { filename, content }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptStatus {
    Neutral,
    Success,
    Error,
}

pub struct Shell {
    history: Vec<String>,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    pub fn listen<F>(&mut self, mut run: F) -> io::Result<()>
    where
        F: FnMut(Command) -> CommandResult,
    {
        while let Some(line) = self.listen_once()? {
            let result = self.resolve_result(run(line.command));
            let status = if result.is_success() {
                PromptStatus::Success
            } else {
                PromptStatus::Error
            };

            self.finish_line(&line.input, status)?;

            if let Some(output) = result.output() {
                if !output.is_empty() {
                    println!("{output}");
                }
            }

            if result.should_exit() {
                break;
            }

            println!();
        }

        Ok(())
    }

    fn resolve_result(&self, result: CommandResult) -> CommandResult {
        let CommandResult::Save { filename, content } = result else {
            return result;
        };

        match self.save_session(&filename, &content) {
            Ok(()) => CommandResult::success(format!("Saved session to `{filename}`")),
            Err(err) => CommandResult::error(format!("Could not save `{filename}`: {err}")),
        }
    }

    fn save_session(&self, filename: &str, content: &str) -> io::Result<()> {
        let mut output = String::new();

        output.push_str("# rnum session\n\n");
        output.push_str("[history]\n");
        for item in &self.history {
            output.push_str(item);
            output.push('\n');
        }

        output.push('\n');
        output.push_str(content);

        fs::write(filename, output)
    }

    fn listen_once(&mut self) -> io::Result<Option<Line>> {
        loop {
            let Some(input) = self.read_line()? else {
                return Ok(None);
            };

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            self.history.push(input.to_string());

            if let Some(command) = parse_command(input) {
                return Ok(Some(Line::new(command, input.to_string())));
            }

            return Ok(Some(Line::new(
                Command::Eval(input.to_string()),
                input.to_string(),
            )));
        }
    }

    fn finish_line(&self, input: &str, status: PromptStatus) -> io::Result<()> {
        print!("\x1b[1F\r\x1b[K{}{}", colored_prompt(status), input);
        print!("\x1b[1E\r");
        io::stdout().flush()
    }

    fn read_line(&mut self) -> io::Result<Option<String>> {
        let _raw_mode = match RawMode::enter() {
            Ok(raw_mode) => raw_mode,
            Err(_) => return self.read_line_fallback(),
        };

        let prompt = colored_prompt(PromptStatus::Neutral);

        print!("{prompt}");
        io::stdout().flush()?;

        let mut editor = LineEditor::new(prompt, &self.history);
        let mut stdin = io::stdin();

        loop {
            match read_control(&mut stdin)? {
                Control::Submit => {
                    print!("\r\n");
                    io::stdout().flush()?;
                    return Ok(Some(editor.input()));
                }
                Control::Cancel => {
                    print!("\r\n");
                    io::stdout().flush()?;
                    return Ok(None);
                }
                control => editor.apply(control)?,
            }
        }
    }

    fn read_line_fallback(&self) -> io::Result<Option<String>> {
        print!("{}", colored_prompt(PromptStatus::Neutral));
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input)? {
            0 => Ok(None),
            _ => Ok(Some(input)),
        }
    }
}

impl CommandResult {
    fn is_success(&self) -> bool {
        matches!(self, CommandResult::Success(_) | CommandResult::Exit)
    }

    fn should_exit(&self) -> bool {
        matches!(self, CommandResult::Exit)
    }

    fn output(&self) -> Option<&str> {
        match self {
            CommandResult::Success(output) | CommandResult::Error(output) => Some(output),
            CommandResult::Save { .. } => None,
            CommandResult::Exit => None,
        }
    }
}

pub struct Line {
    pub command: Command,
    pub input: String,
}

impl Line {
    fn new(command: Command, input: String) -> Self {
        Self { command, input }
    }
}

fn colored_prompt(status: PromptStatus) -> String {
    let color = match status {
        PromptStatus::Neutral => ORANGE,
        PromptStatus::Success => GREEN,
        PromptStatus::Error => RED,
    };

    format!("{color}{PROMPT}{RESET}")
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_command(input: &str) -> Option<Command> {
    if let Some(expr) = input.strip_prefix(":ast") {
        return Some(Command::Ast(expr.trim().to_string()));
    }

    if let Some(name) = input.strip_prefix(":var del ") {
        return Some(Command::Var(VarCommand::Del(name.trim().to_string())));
    }

    if let Some(name) = input.strip_prefix(":fn del ") {
        return Some(Command::Fn(FnCommand::Del(name.trim().to_string())));
    }

    if input == ":save" {
        return Some(Command::Save(String::new()));
    }

    if let Some(filename) = input.strip_prefix(":save ") {
        return Some(Command::Save(filename.trim().to_string()));
    }

    match input {
        ":help" => Some(Command::Help),
        ":exit" | ":quit" => Some(Command::Exit),
        ":var" => Some(Command::Var(VarCommand::List)),
        ":fn" => Some(Command::Fn(FnCommand::List)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Prev,
    Next,
    Left,
    Right,
    Backspace,
    Submit,
    Cancel,
    Insert(char),
    Ignore,
}

struct LineEditor<'a> {
    prompt: String,
    history: &'a [String],
    history_cursor: usize,
    input: Vec<char>,
    cursor: usize,
}

impl<'a> LineEditor<'a> {
    fn new(prompt: String, history: &'a [String]) -> Self {
        Self {
            prompt,
            history,
            history_cursor: history.len(),
            input: Vec::new(),
            cursor: 0,
        }
    }

    fn input(&self) -> String {
        self.input.iter().collect()
    }

    fn apply(&mut self, control: Control) -> io::Result<()> {
        match control {
            Control::Prev => self.prev_history(),
            Control::Next => self.next_history(),
            Control::Left => self.move_left(),
            Control::Right => self.move_right(),
            Control::Backspace => self.backspace(),
            Control::Insert(ch) => self.insert(ch),
            Control::Ignore | Control::Submit | Control::Cancel => {}
        }

        self.render()
    }

    fn prev_history(&mut self) {
        if !self.history.is_empty() && self.history_cursor > 0 {
            self.history_cursor -= 1;
            self.replace_input(&self.history[self.history_cursor]);
        }
    }

    fn next_history(&mut self) {
        if self.history_cursor < self.history.len() {
            self.history_cursor += 1;
            let text = self
                .history
                .get(self.history_cursor)
                .map(String::as_str)
                .unwrap_or_default();
            self.replace_input(text);
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    fn insert(&mut self, ch: char) {
        self.input.insert(self.cursor, ch);
        self.cursor += 1;
    }

    fn replace_input(&mut self, text: &str) {
        self.input = text.chars().collect();
        self.cursor = self.input.len();
    }

    fn render(&self) -> io::Result<()> {
        let text: String = self.input.iter().collect();
        let chars_after_cursor = self.input.len().saturating_sub(self.cursor);

        print!("\r{}{}\x1b[K", self.prompt, text);
        if chars_after_cursor > 0 {
            print!("\x1b[{chars_after_cursor}D");
        }

        io::stdout().flush()
    }
}

fn read_control<R: Read>(reader: &mut R) -> io::Result<Control> {
    let mut byte = [0; 1];
    if reader.read(&mut byte)? == 0 {
        return Ok(Control::Cancel);
    }

    match byte[0] {
        b'\r' | b'\n' => Ok(Control::Submit),
        3 | 4 => Ok(Control::Cancel),
        8 | 127 => Ok(Control::Backspace),
        b'\x1b' => read_escape_control(reader),
        byte if byte.is_ascii_control() => Ok(Control::Ignore),
        byte => Ok(Control::Insert(byte as char)),
    }
}

fn read_escape_control<R: Read>(reader: &mut R) -> io::Result<Control> {
    let mut first = [0; 1];
    if reader.read(&mut first)? == 0 {
        return Ok(Control::Ignore);
    }

    let mut sequence = vec![first[0]];
    if !matches!(first[0], b'[' | b'O') {
        return Ok(Control::Ignore);
    }

    for _ in 0..8 {
        let mut byte = [0; 1];
        if reader.read(&mut byte)? == 0 {
            break;
        }

        sequence.push(byte[0]);
        if (0x40..=0x7e).contains(&byte[0]) {
            break;
        }
    }

    match sequence.as_slice() {
        [b'[', b'A'] | [b'O', b'A'] => Ok(Control::Prev),
        [b'[', b'B'] | [b'O', b'B'] => Ok(Control::Next),
        [b'[', b'C'] | [b'O', b'C'] => Ok(Control::Right),
        [b'[', b'D'] | [b'O', b'D'] => Ok(Control::Left),
        _ => Ok(Control::Ignore),
    }
}

struct RawMode;

impl RawMode {
    fn enter() -> io::Result<Self> {
        let status = ProcessCommand::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            Ok(Self)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "stdin is not a terminal",
            ))
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = ProcessCommand::new("stty")
            .args(["sane"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}
