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

const DEFAULT_SETTINGS_FILE_STEM: &str = "storygame";

impl Settings {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let content = Settings::read_to_string(&path)?;

        let (mut settings, path) = match serde_yaml::from_str::<Settings>(&content) {
            Ok(cfg) => (cfg, path),
            // If given path fails parsing, look for another file in the same directory with
            // the case insensitive file stem `storygame`.
            Err(err) => match path.parent() {
                Some(parent) => {
                    let path = parent
                        .read_dir()
                        .ok()
                        .and_then(|mut entries| {
                            entries.find_map(|entry| {
                                let p = entry.ok()?.path();
                                if p.is_file()
                                    && p.file_stem()?.to_str()?.to_ascii_lowercase()
                                        == DEFAULT_SETTINGS_FILE_STEM
                                {
                                    return Some(p);
                                }
                                None
                            })
                        })
                        .ok_or_else(|| Settings::err_no_read(path, err))?;
                    let content = Settings::read_to_string(&path)?;
                    let cfg = serde_yaml::from_str(&content)
                        .map_err(|e| Settings::err_no_read(&path, e))?;
                    (cfg, path)
                }
                None => return Err(Settings::err_no_read(path, err)),
            },
        };

        if let Some(ext) = settings.logger.file_ext.as_mut() {
            *ext = ext.trim_start_matches('.').to_owned();
        }
        settings.source = Some(path);

        Ok(settings)
    }

    fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String, Error> {
        fs::read_to_string(&path).map_err(|e| Error::read_error(Doctype::Settings, &path).join(e))
    }

    fn err_no_read<P: AsRef<Path>>(path: P, error: serde_yaml::Error) -> Error {
        Error::errors(vec![
            Error::read_error(Doctype::Settings, shorten_path(path)),
            Error::Deserialize(error),
            Error::expected("a valid settings file"),
            Error::message(
                "note: if the settings file is named 'Storygame' or 'Storygame.*' \
                (case insensitive), you can hit <Enter> anywhere in the same directory",
            ),
        ])
        .unwrap()
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
