use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, read_to_string},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use globset::GlobSet;
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

pub struct Output {
    pub files: usize,
    pub code: usize,
    pub unsafe_: usize, // double-counted in `code`
    pub comment: usize,
    pub blank: usize,
}

impl Output {
    fn merge(&mut self, other: &Output) {
        self.files += other.files;
        self.code += other.code;
        self.unsafe_ += other.unsafe_;
        self.comment += other.comment;
        self.blank += other.blank;
    }
}

fn count(path: &Path, lang: &Language) -> Result<Output, CountError> {
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

    Ok(Output {
        files: 1,
        code,
        unsafe_,
        comment,
        blank,
    })
}

pub fn walk(config: &Config) -> Result<HashMap<LanguageId, Output>, CountError> {
    let mut output = HashMap::<LanguageId, Output>::new();

    let iter: &mut dyn Iterator<Item = _> = &mut WalkDir::new(&config.root)
        .into_iter()
        .filter_entry(|entry| {
            // `as_encoded_bytes` returns a "self-synchronizing superset of UTF-8"
            if config.ignore_hidden && entry.file_name().as_encoded_bytes().starts_with(&[b'.']) {
                return false;
            }
            !config.exclude.is_match(entry.path())
        });

    for entry in iter {
        let entry = entry?;

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
                    let file_counts = count(entry.path(), lang)?;
                    match output.entry(lang_id) {
                        Entry::Occupied(mut occupied_entry) => {
                            occupied_entry.get_mut().merge(&file_counts);
                        }
                        Entry::Vacant(vacant_entry) => {
                            vacant_entry.insert(file_counts);
                        }
                    }
                }
            }
        }
    }

    Ok(output)
}
