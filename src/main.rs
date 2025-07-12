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
use count::{Config, CountError, OutputCounts, walk};
use globset::{Glob, GlobSetBuilder};
use languages::{Languages, LanguagesError};
use table::make_table;
use thiserror::Error;

// === Commands ===

#[derive(FromArgs, PartialEq, Debug)]
#[argh(help_triggers("-h", "--help"))]
///
struct Cargo {
    #[argh(subcommand)]
    countlines: Countlines,
}

#[derive(FromArgs, PartialEq, Debug)]
///
#[argh(subcommand, name = "countlines")]
struct Countlines {
    #[argh(positional)]
    path: Option<String>,

    #[argh(
        option,
        short = 'e',
        description = "comma-separated list of files and directories to exclude, both absolute and relative paths are supported"
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
    })
}

fn print(output: OutputCounts, languages: &Languages, time: Duration) {
    let table = make_table(&output, languages);
    println!("{table}");

    println!("{} files errored", output.error_files);
    println!("results in {:?}", time);
}

fn main_() -> Result<(), AppError> {
    env_logger::init();

    let Cargo { countlines: args } = argh::from_env();

    let config = parse_args(&args)?;

    let start = Instant::now();
    let output = walk(&config)?;
    let time = start.elapsed();

    print(output, &config.languages, time);

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
