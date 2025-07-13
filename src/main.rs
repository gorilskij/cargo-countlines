mod count;
mod languages;
mod table;
mod util;

use std::{
    borrow::Cow,
    env::current_dir,
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use argh::FromArgs;
use count::{Config, CountError, OutputCounts, run_count};
use globset::{Glob, GlobSetBuilder};
use languages::{Languages, LanguagesError};
use table::make_table;
use thiserror::Error;

// === Commands ===

#[derive(FromArgs, PartialEq, Debug)]
#[argh(help_triggers("-h", "--help"))]
/// When calling from `cargo countlines`, the whole command is included
/// so we must handle the `cargo` part
struct Cargo {
    #[argh(subcommand)]
    countlines: Countlines,
}

#[derive(FromArgs, PartialEq, Debug)]
/// The actual `countlines` command
#[argh(subcommand, name = "countlines")]
struct Countlines {
    #[argh(
        positional,
        description = "the path to be recursively analyzed, can be absolute or relative"
    )]
    path: Option<String>,

    #[argh(
        option,
        short = 'e',
        description = "files and directories to exclude, supports standard unix glob syntax"
    )]
    exclude: Vec<String>,

    #[argh(
        switch,
        short = 'H',
        description = "ignore hidden files and directories (names starting with .)"
    )]
    ignore_hidden: bool,

    #[argh(
        switch,
        short = 'q',
        description = "do not print progress information while counting lines"
    )]
    quiet: bool,

    #[argh(
        option,
        short = 'd',
        description = "the maximum directory depth to analyze"
    )]
    max_depth: Option<usize>,

    #[argh(switch, short = 'l', description = "follow symbolic links")]
    follow_links: bool,

    #[argh(
        switch,
        short = 'm',
        description = "machine-readable output, without any fancy graphics or extra information"
    )]
    machine_readable: bool,
}

// === Errors ===

#[derive(Error, Debug)]
enum ArgumentError {
    #[error("specified path does not exist: {0}")]
    NonexistentPath(String),
}

#[derive(Error, Debug)]
enum AppError {
    #[error("argument error")]
    ArgumentError(#[from] ArgumentError),

    #[error("globset error")]
    GlobSetError(#[from] globset::Error),

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("languages error")]
    LanguagesError(#[from] LanguagesError),

    #[error("count error")]
    CountError(#[from] CountError),
}

// === Main ===

fn relativize_path(path: Cow<Path>) -> Cow<Path> {
    // try to relativize the path, if anything fails, just treat it as unrelated to cwd
    assert!(path.is_absolute());
    if let Some(rel_path) = current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok())
    {
        Path::new(".").join(rel_path).into()
    } else {
        path
    }
}

fn add_rel_dot(path: Cow<Path>) -> Cow<Path> {
    if path.is_absolute() || path.starts_with(".") || path.starts_with("..") {
        return path;
    }
    Path::new(".").join(path).into()
}

fn parse_args(args: &Countlines) -> Result<Config, AppError> {
    let (abs_root, rel_root) = match &args.path {
        Some(path) => {
            let input_root = PathBuf::from(&path);
            if !input_root.exists() {
                return Err(ArgumentError::NonexistentPath(path.to_string()).into());
            }
            if input_root.is_absolute() {
                let rel_root = relativize_path((&input_root).into()).into_owned();
                (input_root, rel_root)
            } else {
                let abs_root = current_dir()?.join(&input_root);
                let rel_root = add_rel_dot(input_root.into()).into_owned();
                (abs_root, rel_root)
            }
        }
        None => {
            let cwd = current_dir()?;
            let dot = Path::new(".").to_owned();
            (cwd, dot)
        }
    };

    let languages = Languages::load("language_packs/default.json")?;

    let mut builder = GlobSetBuilder::new();
    for pattern in &args.exclude {
        let pattern_path = Path::new(pattern);
        if pattern_path.is_absolute() {
            builder.add(Glob::new(pattern)?);
        } else {
            let mut abs_pattern = abs_root.clone();
            abs_pattern.push(pattern);
            builder.add(Glob::new(
                abs_pattern
                    .as_os_str()
                    .to_str()
                    .expect("non UTF-8 paths are not supported"),
            )?);
        }
    }
    let exclude = builder.build()?;

    Ok(Config {
        abs_root,
        rel_root,
        languages,
        exclude,
        ignore_hidden: args.ignore_hidden,
        quiet: args.quiet,
        max_depth: args.max_depth,
        follow_links: args.follow_links,
        machine_readable: args.machine_readable,
    })
}

fn print(output: OutputCounts, config: &Config, time: Duration) {
    let table = make_table(&output, &config);
    println!("{table}");

    if !config.machine_readable {
        println!("{} files errored", output.error_files);
        println!("results in {:?}", time);
    }
}

fn main_() -> Result<(), AppError> {
    env_logger::init();

    let Cargo { countlines: args } = argh::from_env();

    let config = parse_args(&args)?;

    let start = Instant::now();
    let output = run_count(&config)?;
    let time = start.elapsed();

    print(output, &config, time);

    Ok(())
}

// unstable feature `error_iter`
pub struct Source<'a> {
    current: Option<&'a (dyn Error + 'static)>,
}

impl<'a> Iterator for Source<'a> {
    type Item = &'a (dyn Error + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        self.current = self.current.and_then(Error::source);
        current
    }
}

fn main() {
    match main_() {
        Err(err) => {
            println!("{err}");
            let mut sources = Source {
                current: err.source(),
            }
            .peekable();

            while let Some(err) = sources.next() {
                if sources.peek().is_some() {
                    println!("┣ {err}")
                } else {
                    println!("┗ {err}")
                }
            }
        }
        _ => {}
    }
}
