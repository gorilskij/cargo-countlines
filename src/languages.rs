use std::{collections::HashSet, fs::File, ops::Index, path::Path};

use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LanguagesError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("serde json error")]
    SerdeJson(#[from] serde_json::Error),

    #[error("extension \"{0}\" has the wrong format")]
    WrongFormat(&'static str),

    #[error("extension \"{0}\" used twice")]
    ExtensionUsedTwice(String),
}

#[derive(Deserialize)]
pub struct Language {
    pub name: String,
    pub extensions: Box<[String]>,
    pub line_comments: Option<Box<[String]>>,
}

pub type LanguageId = usize;

// Once created, the Languages struct is immutable
// Each language has an id equivalent to its position in the slice
pub struct Languages {
    languages: Box<[Language]>,
}

impl Languages {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Languages, LanguagesError> {
        let languages: Box<[Language]> = serde_json::from_reader(File::open(path)?)?;
        Languages::from(languages)
    }

    pub fn from(languages: Box<[Language]>) -> Result<Languages, LanguagesError> {
        let mut extensions = HashSet::new();
        for lang in &languages {
            for ext in &lang.extensions {
                if ext.chars().count() < 2 {
                    return Err(LanguagesError::WrongFormat("extension empty"));
                }

                if !ext.starts_with('.') {
                    return Err(LanguagesError::WrongFormat(
                        "extension doesn't start with a dot",
                    ));
                }

                if ext.chars().skip(1).any(|c| c == '.') {
                    return Err(LanguagesError::WrongFormat("extension contains a dot"));
                }

                if !extensions.contains(&ext) {
                    extensions.insert(ext);
                } else {
                    return Err(LanguagesError::ExtensionUsedTwice(ext.to_string()));
                }
            }
        }

        Ok(Languages { languages })
    }
}

impl Index<LanguageId> for Languages {
    type Output = Language;

    fn index(&self, index: LanguageId) -> &Self::Output {
        &self.languages[index]
    }
}

impl<'a> IntoIterator for &'a Languages {
    type Item = &'a Language;

    type IntoIter = std::slice::Iter<'a, Language>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.languages).into_iter()
    }
}
