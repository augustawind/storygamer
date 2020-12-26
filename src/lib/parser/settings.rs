use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use log::LevelFilter;
use serde::de;
use serde::Deserialize;

use crate::errors::{Doctype, Error};
use crate::types::{item, ItemDef, Variable};
use crate::utils::shorten_path;

use super::PageID;

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
    #[serde(default, deserialize_with = "deserialize_item_defs")]
    items: HashMap<String, ItemDef>,
    logger: LoggingSettings,
}

fn deserialize_item_defs<'de, D>(deserializer: D) -> Result<HashMap<String, ItemDef>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct ItemDefsVisitor;

    impl<'de> de::Visitor<'de> for ItemDefsVisitor {
        type Value = HashMap<String, ItemDef>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("object mapping item names to item definitions")
        }

        fn visit_map<A: de::MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut defs = HashMap::with_capacity(access.size_hint().unwrap_or(0));
            while let Some((name, mut item_def)) = access.next_entry::<String, ItemDef>()? {
                // If given, ensure `max_uses` value is in range.
                if let Some(max_uses) = item_def.max_uses {
                    if !item::ITEM_USES_RANGE.contains(&max_uses) {
                        return Err(de::Error::invalid_value(
                            de::Unexpected::Signed(max_uses as i64),
                            &format!(
                                "an integer between {} and {}",
                                item::ITEM_USES_RANGE.start(),
                                item::ITEM_USES_RANGE.end()
                            )
                            .as_str(),
                        ));
                    }
                }

                // Set the `name` for each [`ItemDef`] from the map key it's under.
                if !item_def.name.is_empty() {
                    return Err(de::Error::unknown_field("name", item::ITEM_DEF_FIELDS));
                }
                item_def.name = name.clone();

                defs.insert(name, item_def);
            }
            Ok(defs)
        }
    }

    deserializer.deserialize_map(ItemDefsVisitor)
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
        Error::parse_error(Doctype::Settings, shorten_path(path), error).join(Error::message(
            "note: if the settings file is named 'Storygame' or 'Storygame.*' \
            (case insensitive), you can hit <Enter> anywhere in the same directory",
        ))
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
    pub fn items(&self) -> &HashMap<String, ItemDef> {
        &self.items
    }
    pub fn logger(&self) -> &LoggingSettings {
        &self.logger
    }
}
