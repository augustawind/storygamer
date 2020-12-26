use std::cmp::Ordering;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use cursive::align::{Align, HAlign};
use cursive::event::Event;
use cursive::traits::{Nameable, Resizable, Scrollable};
use cursive::views::{Dialog, LinearLayout, OnEventView, Panel, SelectView, TextView};
use cursive::Cursive;

use crate::app::{logger::LogConfig, AppState, Game};
use crate::parser::{self, Settings};
use crate::utils::is_parent_path;

use super::{on_menu_back, redraw_content};

mod constants {
    pub mod container {
        pub const TITLE: &str = "Select Project from Files";
        pub const PADDING_TOP: usize = 1;
        pub const MAX_HEIGHT: usize = 30;
        pub const MIN_WIDTH: usize = 50;
    }

    pub mod curdir_display {
        pub const NAME: &str = "curdir_display";
        pub const MAX_CONTENT_LEN: usize = 35;
    }

    pub mod file_select {
        pub const NAME: &str = "file_select";
    }
}

pub fn open(siv: &mut Cursive) {
    let select = SelectView::<PathBuf>::new()
        .on_submit(|s: &mut Cursive, path: &PathBuf| {
            // If a directory was selected, expand it.
            if path.is_dir() {
                update_file_select(s, path);
            // Otherwise a file was selected; attempt to open it.
            } else {
                load_storygame(s, path);
                s.pop_layer();
                redraw_content(s);
            }
        })
        .autojump()
        .with_name(constants::file_select::NAME)
        .scrollable();

    let view = OnEventView::new(
        Dialog::around(
            LinearLayout::vertical()
                .child(TextView::empty().with_name(constants::curdir_display::NAME))
                .child(Panel::new(select).full_height()),
        )
        .title(constants::container::TITLE)
        .padding_top(constants::container::PADDING_TOP)
        .max_height(constants::container::MAX_HEIGHT)
        .min_width(constants::container::MIN_WIDTH),
    )
    .on_event(Event::CtrlChar('b'), on_menu_back);

    siv.add_layer(view);

    update_file_select(siv, Path::new(".").canonicalize().unwrap());
}

pub fn load_storygame<P: AsRef<Path>>(siv: &mut Cursive, path: P) {
    let settings = unwrap_or_notify!(siv, Settings::read(path));
    debug!("loading storygame: parsed settings");

    let starting_page = unwrap_or_notify!(siv, parser::parse(&settings));
    let game = Game::new(&starting_page, settings.variables(), settings.items());
    debug!("loading storygame: parsed game");

    // Update app state.
    unwrap_or_notify!(
        siv,
        siv.with_user_data(|app: &mut AppState| {
            app.game.replace(game);
            let log = settings.logger();
            let default = LogConfig::default();
            app.logger.set_config(LogConfig {
                base_file_name: log
                    .base_file_name
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| default.base_file_name),
                file_ext: log
                    .file_ext
                    .as_ref()
                    .map(|s| s.as_str())
                    .or_else(|| default.file_ext)
                    .and_then(|s| match s {
                        "" => None,
                        s => Some(s),
                    }),
                level: log.level.unwrap_or_else(|| default.level),
                ..Default::default()
            })
        })
        .transpose()
    );
    debug!("loading storygame: complete");
}

pub fn close(siv: &mut Cursive) {
    siv.with_user_data(|app: &mut AppState| app.game = None);
    redraw_content(siv);
}

fn update_file_select<S: AsRef<Path>>(s: &mut Cursive, path: S) {
    let path = path.as_ref();
    let entries = path.read_dir().unwrap();

    s.call_on_name(constants::curdir_display::NAME, |view: &mut TextView| {
        let display_path =
            if path.to_str().unwrap().len() > constants::curdir_display::MAX_CONTENT_LEN {
                let path_parts: Vec<&OsStr> = path.iter().collect();

                let mut total_len = 1;
                let mut to_idx = path_parts.len() + 1;
                for part in path_parts.iter().skip(1).rev() {
                    let part = part.to_str().unwrap();
                    total_len += part.len();
                    if total_len > constants::curdir_display::MAX_CONTENT_LEN {
                        break;
                    }
                    to_idx -= 1;
                }

                Path::new("..").join(
                    path.strip_prefix(path_parts[..to_idx].into_iter().collect::<PathBuf>())
                        .unwrap(),
                )
            } else {
                path.to_path_buf()
            };

        view.set_content(format!(" ▶ {}", display_path.to_str().unwrap()));
    });

    s.call_on_name(
        constants::file_select::NAME,
        |select: &mut SelectView<PathBuf>| {
            select.clear();

            if let Some(parent) = path.parent() {
                select.add_item("..", parent.to_path_buf());
            }

            for entry in entries {
                let path = entry.unwrap().path();
                let mut file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                if path.is_dir() {
                    file_name += "/";
                }
                select.add_item(file_name, path);
            }

            select.sort_by(|l: &PathBuf, r: &PathBuf| match (l.is_dir(), r.is_dir()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (true, true) => {
                    if is_parent_path(l, r) {
                        Ordering::Less
                    } else if is_parent_path(r, l) {
                        Ordering::Greater
                    } else {
                        l.cmp(r)
                    }
                }
                (false, false) => l.cmp(r),
            });
        },
    );
}
