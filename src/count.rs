use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, read_to_string},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Output,
};

use globset::GlobSet;
use rayon::iter::{ParallelBridge, ParallelIterator};
use thiserror::Error;
use walkdir::WalkDir;

use crate::languages::{Language, LanguageId, Languages};

#[derive(Error, Debug)]
pub enum CountError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("walkdir error")]
    WalkDir(#[from] walkdir::Error),
}

pub struct Config {
    // absolute path
    pub root: PathBuf,
    pub languages: Languages,
    // all paths are absolute
    pub exclude: GlobSet,
    pub ignore_hidden: bool,
}

#[derive(Clone)]
pub struct Counts {
    pub files: usize,
    pub code: usize,
    pub unsafe_: usize, // double-counted in `code`
    pub comment: usize,
    pub blank: usize,
}

impl Counts {
    fn merge(&mut self, other: &Counts) {
        self.files += other.files;
        self.code += other.code;
        self.unsafe_ += other.unsafe_;
        self.comment += other.comment;
        self.blank += other.blank;
    }
}

fn count(path: &Path, lang: &Language) -> Result<Counts, CountError> {
    let mut code = 0;
    let mut unsafe_ = 0;
    let mut comment = 0;
    let mut blank = 0;

    let line_comments = lang.line_comments.as_ref().map(|lcs| &**lcs).unwrap_or(&[]);

    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            blank += 1;
        }
        // TODO: account for block comments
        else if line_comments.iter().any(|lc| line.starts_with(lc)) {
            comment += 1;
        }
        // TODO: account for unsafe code
        else {
            code += 1;
        }
    }

    Ok(Counts {
        files: 1,
        code,
        unsafe_,
        comment,
        blank,
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

pub fn walk(config: &Config) -> Result<OutputCounts, CountError> {
    let iter = WalkDir::new(&config.root)
        .into_iter()
        .filter_entry(|entry| {
            // `as_encoded_bytes` returns a "self-synchronizing superset of UTF-8"
            if config.ignore_hidden && entry.file_name().as_encoded_bytes().starts_with(&[b'.']) {
                return false;
            }
            !config.exclude.is_match(entry.path())
        });

    let output = iter
        .par_bridge()
        .map(|entry| {
            match entry {
                Ok(entry) => {
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
                                return match count(entry.path(), lang) {
                                    Ok(counts) => EntryResult::Some { lang_id, counts },
                                    Err(err) => EntryResult::Err(err),
                                };
                            }
                        }
                    }

                    EntryResult::None
                }
                Err(err) => EntryResult::Err(err.into()),
            }
        })
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
