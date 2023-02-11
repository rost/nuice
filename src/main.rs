use std::{
    collections::HashMap,
    ffi::OsStr,
    io::{self, Write},
    path::{Path, PathBuf},
};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style,
    terminal::{self, ClearType},
    Result,
};

#[derive(PartialEq)]
enum OpKind {
    Out,
}

struct Op {
    kind: OpKind,
    path: Option<PathBuf>,
}

impl Op {
    fn new(kind: OpKind, path: PathBuf) -> Op {
        Op {
            kind,
            path: Some(path),
        }
    }
}

struct State {
    cursor: i32,
    dir: PathBuf,
    paths: HashMap<PathBuf, i32>,
    prev_op: Option<Op>,
    screen_lines: Vec<String>,
}

impl State {
    fn new() -> Result<State> {
        Ok(State {
            cursor: 0,
            dir: std::env::current_dir()?,
            paths: HashMap::new(),
            prev_op: None,
            screen_lines: format_screen_lines(0, get_dir_content()?)?,
        })
    }
}

fn main() -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    run(&mut stdout)?;
    Ok(())
}

fn run<W>(w: &mut W) -> anyhow::Result<()>
where
    W: Write,
{
    execute!(w, terminal::EnterAlternateScreen)?;

    terminal::enable_raw_mode()?;

    let mut state = State::new()?;

    loop {
        queue!(
            w,
            style::ResetColor,
            terminal::Clear(ClearType::All),
            cursor::Hide,
            cursor::MoveTo(1, 1)
        )?;

        state.dir = std::env::current_dir()?;

        state.cursor = cursor_position(&state)?;

        state.screen_lines = format_screen_lines(state.cursor, get_dir_content()?)?;

        for line in &state.screen_lines {
            queue!(w, style::Print(line), cursor::MoveToNextLine(1))?;
        }

        w.flush()?;

        match read_char()? {
            'q' => break,
            char => handle_keypress(&char, &mut state)?,
        };

        state.paths.insert(state.dir, state.cursor);
    }

    execute!(
        w,
        style::ResetColor,
        cursor::Show,
        terminal::LeaveAlternateScreen
    )?;

    Ok(terminal::disable_raw_mode()?)
}

fn cursor_position(state: &State) -> Result<i32> {
    let cursor = if state.paths.contains_key(&state.dir) {
        match state.paths.get(&state.dir) {
            Some(cursor) => *cursor,
            None => 0,
        }
    } else {
        match &state.prev_op {
            Some(op) if op.kind == OpKind::Out => {
                let last = match op.path.as_ref() {
                    Some(path) => match path.file_name() {
                        Some(v) => v,
                        None => OsStr::new(""),
                    },
                    None => OsStr::new(""),
                };
                let index = get_dir_content()?
                    .iter()
                    .position(|x| x.file_name() == Some(last))
                    .unwrap_or(0);
                index as i32
            }
            Some(_) => 0,
            None => 0,
        }
    };
    Ok(cursor)
}

fn handle_keypress<'a>(char: &char, state: &'a mut State) -> Result<&'a mut State> {
    let state = match char {
        'j' => {
            state.cursor = move_down(state)?;
            state.prev_op = None;
            state
        }
        'k' => {
            state.cursor = move_up(state)?;
            state.prev_op = None;
            state
        }
        'h' => {
            let cursor = move_out_of_dir(state)?;
            let op = Some(Op::new(OpKind::Out, state.dir.clone()));
            state.cursor = cursor;
            state.prev_op = op;
            state
        }
        'l' => {
            state.cursor = move_into_dir(state)?;
            state.prev_op = None;
            state
        }
        _ => state,
    };
    Ok(state)
}

fn get_dir_content() -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(".")? {
        let entry = entry?;
        let path = entry.path();
        entries.push(path);
    }
    entries.sort();
    Ok(entries)
}

fn format_screen_lines(cursor: i32, content: Vec<PathBuf>) -> Result<Vec<String>> {
    let content = match !content.is_empty() {
        true => content,
        false => vec![PathBuf::from("   ../")],
    };

    let mut lines = Vec::new();
    let current_dir = std::env::current_dir()?;
    lines.push(format!("{}", current_dir.display()));
    lines.push(String::from(""));

    for entry in content {
        lines.push(pathbuf_to_string(&entry));
    }

    let index = (cursor + 2) as usize;
    lines[index] = format!(" > {}", lines[index].trim_start());

    Ok(lines)
}

fn pathbuf_to_string(path: &Path) -> String {
    match path.file_name() {
        Some(v) => match v.to_str() {
            Some(v) => match path.is_dir() {
                true => format!("   {v}/"),
                false => format!("   {v}"),
            },
            None => "".to_string(),
        },
        None => "".to_string(),
    }
}

fn read_char() -> Result<char> {
    loop {
        if let Ok(Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            ..
        })) = event::read()
        {
            return Ok(c);
        }
    }
}

fn move_down(state: &State) -> Result<i32> {
    let cursor = if state.cursor + 1 < (state.screen_lines.len() - 2) as i32 {
        state.cursor + 1
    } else {
        state.cursor
    };
    Ok(cursor)
}

fn move_up(state: &State) -> Result<i32> {
    let cursor = if state.cursor > 0 {
        state.cursor - 1
    } else {
        0
    };
    Ok(cursor)
}

fn move_out_of_dir(state: &State) -> Result<i32> {
    std::env::set_current_dir("..")?;
    Ok(state.cursor)
}

fn move_into_dir(state: &State) -> Result<i32> {
    let path = state.screen_lines[(state.cursor + 2) as usize].trim_start();
    let newdir = path.trim_end_matches('/');
    let newdir = str::replace(newdir, ">", " ");
    let newdir = newdir.trim_start();
    let current_dir = std::env::current_dir()?;
    let newdir = current_dir.join(newdir);
    if path.ends_with('/') {
        std::env::set_current_dir(newdir)?;
    }
    Ok(state.cursor)
}
