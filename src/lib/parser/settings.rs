use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use log::LevelFilter;
use serde::Deserialize;

use crate::errors::{Doctype, Error};
use crate::parser::PageID;
use crate::types::Variable;
use crate::utils::shorten_path;

#[derive(Deserialize, Debug, Clone)]
pub struct LoggingSettings {
    #[serde(default)]
    pub base_file_name: Option<String>,
    #[serde(default)]
    pub file_ext: Option<String>,
    #[serde(default)]
    pub level: Option<LevelFilter>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(skip)]
    source: Option<PathBuf>,
    title: String,
    base_dir: PathBuf,
    entrypoint: PathBuf,
    pages: HashSet<PageID>,
    #[serde(default)]
    variables: HashMap<String, Variable>,
    logger: LoggingSettings,
}

impl Settings {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(&path)
            .map_err(|e| Error::read_error(Doctype::Settings, &path).join(e))?;

        let (mut settings, path) = match serde_yaml::from_str::<Settings>(&content) {
            Ok(cfg) => (cfg, path),
            Err(err) => match path.parent() {
                Some(parent) => {
                    let path = parent
                        .read_dir()
                        .ok()
                        .and_then(|mut entries| {
                            entries.find_map(|entry| {
                                let p = entry.ok()?.path();
                                if p.is_file() && p.file_stem()?.to_str()? == "Storygame" {
                                    return Some(p);
                                }
                                None
                            })
                        })
                        .ok_or_else(|| {
                            Error::errors(vec![
                                Error::read_error(Doctype::Settings, shorten_path(path)),
                                Error::expected("a valid settings file"),
                                Error::message(
                                    "note: if the settings file is named `Storygame` or \
                                    `Storygame.*`, you can hit <Enter> anywhere in the directory \
                                    in which it's located",
                                ),
                            ])
                            .unwrap()
                        })?;
                    (serde_yaml::from_str(&fs::read_to_string(&path)?)?, path)
                }
                None => return Err(Error::Deserialize(err)),
            },
        };

        if let Some(ext) = settings.logger.file_ext.as_mut() {
            *ext = ext.trim_start_matches('.').to_owned();
        }
        settings.source = Some(path);

        Ok(settings)
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_ref().map(PathBuf::as_path)
    }
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn base_dir(&self) -> &Path {
        self.base_dir.as_path()
    }
    pub fn entrypoint(&self) -> &Path {
        self.entrypoint.as_path()
    }
    pub fn pages(&self) -> &HashSet<PageID> {
        &self.pages
    }
    pub fn variables(&self) -> &HashMap<String, Variable> {
        &self.variables
    }
    pub fn logger(&self) -> &LoggingSettings {
        &self.logger
    }
}
