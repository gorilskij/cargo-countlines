mod count;
mod languages;

use std::{
    cmp::Ordering,
    env::current_dir,
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use argh::FromArgs;
use count::{Config, CountError, OutputCounts, walk};
use globset::{Glob, GlobSetBuilder};
use languages::{Languages, LanguagesError};
use tabled::settings::Style;
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

fn print(output: OutputCounts, languages: &Languages, time: Duration) {
    let ordered_counts = {
        let mut ordered_counts = output
            .counts
            .iter()
            .map(|(lang_id, counts)| (*lang_id, counts))
            .collect::<Vec<_>>();

        // reverse order by number of code lines, forward order by language
        ordered_counts.sort_unstable_by(|(lang_id1, counts1), (lang_id2, counts2)| {
            match counts2.code.cmp(&counts1.code) {
                Ordering::Equal => lang_id1.cmp(lang_id2),
                ord => ord,
            }
        });
        ordered_counts
    };

    let mut builder = tabled::builder::Builder::default();
    builder.push_record(["", "files", "code", "comment", "blank"]);
    for (lang_id, counts) in ordered_counts {
        builder.push_record([
            languages[lang_id].name.clone(),
            counts.files.to_string(),
            counts.code.to_string(),
            counts.comment.to_string(),
            counts.blank.to_string(),
        ]);
    }

    let mut table = builder.build();
    println!("{}", table.with(Style::rounded()));

    println!("{} files errored", output.error_files);
    println!("results in {:?}", time);
}

fn main_() -> Result<(), AppError> {
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
