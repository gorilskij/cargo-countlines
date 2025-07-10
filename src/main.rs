mod count;
mod languages;

use std::{
    env::current_dir,
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
};

use argh::FromArgs;
use count::{Config, CountError, walk};
use globset::{Glob, GlobSetBuilder};
use languages::{Languages, LanguagesError};
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
        description = "comma-separated list of files and directories to exclude, both absolute and relative paths are supported"
    )]
    exclude: Option<String>,

    #[argh(
        switch,
        description = "ignore hidden files and directories (names starting with .)"
    )]
    ignore_hidden: bool,
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

fn parse_args(args: &Countlines) -> Result<Config, AppError> {
    let root = match &args.path {
        Some(path) => {
            let root = PathBuf::from(&path);
            if !root.exists() {
                return Err(ArgumentError::NonexistentPath(path.to_string()).into());
            }
            if root.is_absolute() {
                root
            } else {
                let mut abs = current_dir()?;
                abs.push(root);
                abs
            }
        }
        None => current_dir()?,
    };

    let languages = Languages::load("language_packs/default.json")?;

    let mut builder = GlobSetBuilder::new();
    if let Some(exclude_list) = &args.exclude {
        for pattern in exclude_list.split(',') {
            let pattern_path = Path::new(pattern);
            if pattern_path.is_absolute() {
                builder.add(Glob::new(pattern)?);
            } else {
                let mut abs = root.clone();
                abs.push(pattern);
                builder.add(Glob::new(
                    abs.as_os_str()
                        .to_str()
                        .expect("non UTF-8 paths are not supported"),
                )?);
            }
        }
    }
    let exclude = builder.build()?;

    Ok(Config {
        root,
        languages,
        exclude,
        ignore_hidden: args.ignore_hidden,
    })
}

fn main_() -> Result<(), AppError> {
    let Cargo { countlines: args } = argh::from_env();

    let config = parse_args(&args)?;

    let start = Instant::now();
    let output = walk(&config)?;
    let time = start.elapsed();

    for (lang_id, counts) in output.counts {
        println!("{}", config.languages[lang_id].name);
        println!("    files {}", counts.files);
        println!("     code {}", counts.code);
        println!("   unsafe {}", counts.unsafe_);
        println!("  comment {}", counts.comment);
        println!("    blank {}", counts.blank);
    }
    println!("{} files errored", output.error_files);
    println!("results in {:?}", time);

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
            println!("{}", err);
            let mut sources = Source {
                current: err.source(),
            }
            .peekable();

            while let Some(err) = sources.next() {
                if sources.peek().is_some() {
                    println!("┣ {}", err)
                } else {
                    println!("┗ {}", err)
                }
            }
        }
        _ => {}
    }
}
