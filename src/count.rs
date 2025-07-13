use std::{
    collections::{HashMap, hash_map::Entry},
    path::{Path, PathBuf},
};

use futures::StreamExt;
use rayon::iter::{ParallelBridge, ParallelIterator};
use split_async::split;
use std::fs::File as StdFile;
use std::io::BufRead as _;
use std::io::BufReader as StdBufReader;
use tokio::io::BufReader as TokioBufReader;
use tokio::{fs::File as TokioFile, io::AsyncBufReadExt, runtime::Runtime};

use globset::GlobSet;
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

use crate::{
    AppError, Mode,
    languages::{Language, LanguageId, Languages},
};

#[derive(Error, Debug)]
pub enum CountError {
    #[error("walkdir error")]
    WalkDir(#[from] walkdir::Error),

    #[error("io error in file {path}")]
    Io { path: PathBuf, err: std::io::Error },
}

pub struct Config {
    pub abs_root: PathBuf,
    pub rel_root: PathBuf, // relative to cwd
    pub languages: Languages,
    pub exclude: GlobSet, // all glob patterns are absolute
    pub ignore_hidden: bool,
    pub quiet: bool,
    pub max_depth: Option<usize>,
    pub follow_links: bool,
    pub machine_readable: bool,
    pub mode: Mode,
}

#[derive(Clone)]
pub struct Counts {
    pub files: usize,
    pub code: usize,
    pub comment: usize,
    pub blank: usize,
    pub invalid: usize,
}

impl Counts {
    fn merge(&mut self, other: &Counts) {
        self.files += other.files;
        self.code += other.code;
        self.comment += other.comment;
        self.blank += other.blank;
        self.invalid += other.invalid;
    }
}

fn sync_count(path: &Path, lang: &Language) -> Result<Counts, std::io::Error> {
    let mut code = 0;
    let mut comment = 0;
    let mut blank = 0;
    let mut invalid = 0;

    let line_comments = lang
        .line_comments
        .as_ref()
        .map(|c| c.as_ref())
        .unwrap_or(&[]);
    let block_comments = lang
        .block_comments
        .as_ref()
        .map(|c| c.as_ref())
        .unwrap_or(&[]);

    let mut in_block_comment = None;
    for line in StdBufReader::new(StdFile::open(path)?).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_err) => {
                invalid += 1;
                continue;
            }
        };
        let line = line.trim();

        if line.is_empty() {
            blank += 1;
            continue;
        }

        if let Some(end_token) = in_block_comment {
            comment += 1;
            if line.ends_with(end_token) {
                in_block_comment = None;
            }
            continue;
        }

        if line_comments.iter().any(|lc| line.starts_with(lc)) {
            comment += 1;
            continue;
        }

        if let Some((_, end_token)) = block_comments
            .iter()
            .find(|(start_token, _)| line.starts_with(start_token))
        {
            if !line.ends_with(end_token) {
                in_block_comment = Some(end_token);
            }
            comment += 1;
            continue;
        }

        code += 1;
    }

    Ok(Counts {
        files: 1,
        code,
        comment,
        blank,
        invalid,
    })
}

async fn async_count(path: &Path, lang: &Language) -> Result<Counts, std::io::Error> {
    let mut code = 0;
    let mut comment = 0;
    let mut blank = 0;
    let mut invalid = 0;

    let line_comments = lang
        .line_comments
        .as_ref()
        .map(|c| c.as_ref())
        .unwrap_or(&[]);
    let block_comments = lang
        .block_comments
        .as_ref()
        .map(|c| c.as_ref())
        .unwrap_or(&[]);

    let mut in_block_comment = None;
    let mut iter = TokioBufReader::new(TokioFile::open(path).await?).lines();
    loop {
        let line = match iter.next_line().await {
            Ok(l) => l,
            Err(_err) => {
                invalid += 1;
                continue;
            }
        };
        let Some(line) = line else { break };
        let line = line.trim();

        if line.is_empty() {
            blank += 1;
            continue;
        }

        if let Some(end_token) = in_block_comment {
            comment += 1;
            if line.ends_with(end_token) {
                in_block_comment = None;
            }
            continue;
        }

        if line_comments.iter().any(|lc| line.starts_with(lc)) {
            comment += 1;
            continue;
        }

        if let Some((_, end_token)) = block_comments
            .iter()
            .find(|(start_token, _)| line.starts_with(start_token))
        {
            if !line.ends_with(end_token) {
                in_block_comment = Some(end_token);
            }
            comment += 1;
            continue;
        }

        code += 1;
    }

    Ok(Counts {
        files: 1,
        code,
        comment,
        blank,
        invalid,
    })
}

enum EntryResult {
    Some { lang_id: LanguageId, counts: Counts },
    None, // file didn't match
    Err(CountError),
}

#[derive(Default)]
pub struct OutputCounts {
    pub counts: HashMap<LanguageId, Counts>,
    pub unmatched_files: usize,
    pub error_files: usize,
}

impl OutputCounts {
    fn append_counts(&mut self, lang_id: LanguageId, counts: &Counts) {
        match self.counts.entry(lang_id) {
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().merge(counts);
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(counts.clone());
            }
        }
    }

    fn merge(&mut self, other: &Self) {
        for (lang_id, counts) in &other.counts {
            self.append_counts(*lang_id, counts);
        }
        self.unmatched_files += other.unmatched_files;
        self.error_files += other.error_files;
    }
}

// === Walk internals ===

fn make_walk_iter(config: &Config) -> impl Iterator<Item = Result<DirEntry, walkdir::Error>> {
    let mut iter = WalkDir::new(&config.abs_root);
    if let Some(max_depth) = config.max_depth {
        iter = iter.max_depth(max_depth);
    }
    if config.follow_links {
        iter = iter.follow_links(true);
    }
    let iter = iter.into_iter().filter_entry(|entry| {
        // `as_encoded_bytes` returns a "self-synchronizing superset of UTF-8"
        if config.ignore_hidden && entry.file_name().as_encoded_bytes().starts_with(&[b'.']) {
            return false;
        }
        !config.exclude.is_match(entry.path())
    });

    iter
}

#[split]
async fn walk_loop_body(
    entry: Result<DirEntry, walkdir::Error>,
    config: &Config,
    pbar: Option<&ProgressBar>,
) -> EntryResult {
    let entry = match entry {
        Ok(e) if e.file_type().is_file() => e,
        Ok(_) => return EntryResult::None, // dir or symlink
        Err(err) => return EntryResult::Err(err.into()),
    };

    info!("{:?}", entry.path());
    pbar.map(|pbar| {
        pbar.inc(1);

        // display path relative to cwd
        // default to absolute path if `stip_prefix` fails
        let display_path = entry
            .path()
            .strip_prefix(&config.abs_root)
            .map(|rel_path| config.rel_root.join(rel_path).to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

        pbar.set_message(display_path);
    });

    for (lang_id, lang) in (&config.languages).into_iter().enumerate() {
        for ext in &lang.extensions {
            // `as_encoded_bytes` returns a "self-synchronizing superset of UTF-8"
            // This means that if the last few bytes match the ASCII values for a file extension,
            // then we can safely assume that's what they are
            if entry
                .file_name()
                .as_encoded_bytes()
                .ends_with(ext.as_bytes())
            {
                let counts: Result<_, _> = choose!(count)(entry.path(), lang).await;
                return match counts {
                    Ok(counts) => EntryResult::Some { lang_id, counts },
                    Err(err) => {
                        warn!("error in file {:?}", entry.path());
                        EntryResult::Err(CountError::Io {
                            path: entry.into_path(),
                            err,
                        })
                    }
                };
            }
        }
    }

    EntryResult::None
}

fn sync_walk(config: &Config, pbar: Option<&ProgressBar>) -> Result<OutputCounts, CountError> {
    let iter = make_walk_iter(config);

    let output = iter
        .map(|entry| sync_walk_loop_body(entry, config, pbar))
        .fold(OutputCounts::default(), |mut output, entry_result| {
            match entry_result {
                EntryResult::Some { lang_id, counts } => output.append_counts(lang_id, &counts),
                EntryResult::None => output.unmatched_files += 1,
                EntryResult::Err(_err) => output.error_files += 1,
            }
            output
        });

    Ok(output)
}

async fn async_walk(
    config: &Config,
    pbar: Option<&ProgressBar>,
) -> Result<OutputCounts, CountError> {
    let iter = make_walk_iter(config);

    let output = futures::stream::iter(iter)
        .map(|entry| async_walk_loop_body(entry, config, pbar))
        .buffer_unordered(20)
        .fold(OutputCounts::default(), async |mut output, entry_result| {
            match entry_result {
                EntryResult::Some { lang_id, counts } => output.append_counts(lang_id, &counts),
                EntryResult::None => output.unmatched_files += 1,
                EntryResult::Err(_err) => output.error_files += 1,
            }
            output
        })
        .await;

    Ok(output)
}

fn parallel_walk(config: &Config, pbar: Option<&ProgressBar>) -> Result<OutputCounts, CountError> {
    let iter = make_walk_iter(config);

    let output = iter
        .par_bridge()
        .map(|entry| sync_walk_loop_body(entry, config, pbar))
        .fold(
            || OutputCounts::default(),
            |mut output, entry_result| {
                match entry_result {
                    EntryResult::Some { lang_id, counts } => output.append_counts(lang_id, &counts),
                    EntryResult::None => output.unmatched_files += 1,
                    EntryResult::Err(_err) => output.error_files += 1,
                }
                output
            },
        )
        .reduce(
            || OutputCounts::default(),
            |mut output1, output2| {
                output1.merge(&output2);
                output1
            },
        );

    Ok(output)
}

pub fn run_count(config: &Config) -> Result<OutputCounts, AppError> {
    let rt = Runtime::new()?;

    let pbar = (!config.quiet).then(|| {
        let pbar = ProgressBar::no_length();
        pbar.set_style(
            ProgressStyle::with_template("[{elapsed_precise}] {human_pos} {msg}").unwrap(),
        );
        pbar
    });

    let output = match config.mode {
        Mode::Sync => sync_walk(config, pbar.as_ref()),
        Mode::Async => {
            let async_output = async_walk(config, pbar.as_ref());
            rt.block_on(async_output)
        }
        Mode::Parallel => parallel_walk(config, pbar.as_ref()),
    };

    pbar.as_ref().map(|pbar| pbar.finish_and_clear());

    Ok(output?)
}
