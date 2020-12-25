use std::iter;
use std::rc::Rc;

use cursive::align::HAlign;
use cursive::event::{Event, Key};
use cursive::menu::MenuTree;
use cursive::theme::{BaseColor, Color, Effect, Style};
use cursive::traits::*;
use cursive::utils::markup::{markdown, StyledString};
use cursive::view::{scroll::Scroller, Margins, Scrollable};
use cursive::views::{
    Dialog, DummyView, EditView, LinearLayout, ListView, OnEventView, PaddedView, Panel,
    ScrollView, SelectView, TextView,
};
use cursive::{Cursive, Rect};
use handlebars::Handlebars;

use crate::app::{logger::LogView, AppState, Game};
use crate::types::{Prompt, Variable};

macro_rules! unwrap_or_notify {
    ($siv:expr, $expr:expr) => {{
        match $expr {
            Ok(val) => val,
            Err(err) => {
                error!("{}", err.to_string());
                $siv.add_layer(
                    Dialog::around(TextView::new(err.to_string_verbose()).align(Align::center()))
                        .h_align(HAlign::Center)
                        .title("Error")
                        .dismiss_button("OK")
                        .max_width(80),
                );
                return;
            }
        }
    }};
}

mod menu;

// Globals and constants for various UI components.
mod constants {
    pub mod labels {
        pub const FILE: &str = "File";
        pub const HELP: &str = "Help (^H)";

        pub const NEXT: &str = "Next (^N)";
        pub const BACK: &str = "Back (^B)";
        pub const QUIT: &str = "Quit (^Q)";
    }

    pub mod help {
        pub mod commands {
            pub const GENERAL: &[(&str, &str)] = &[
                ("Focus next element", "<Tab>"),
                ("Focus previous element", "<S-Tab>"),
                ("Focus menubar", "<Esc>"),
                ("Show help", "^H"),
                ("Quit", "^Q"),
            ];
            pub const NAVIGATION: &[(&str, &str)] = &[
                ("Scroll up/down", "↑/↓, k/j"),
                ("Scroll up/down half a page", "^U / ^D"),
                ("Scroll down a page", "<Space>"),
                ("Scroll to beginning", "g, <Home>"),
                ("Scroll to end", "G, <End>"),
                ("Goto next section", "^N"),
                ("Goto previous section", "^P"),
            ];
        }
    }
}

/// The app's main entrypoint.
///
/// Calling this takes over the terminal, creating the TUI, and starts listening for user input.
pub fn run() {
    let mut siv = cursive::default();

    let app_state = AppState::new().unwrap();
    siv.set_user_data::<AppState>(app_state);

    siv.add_global_callback(Key::Esc, |s| s.select_menubar());
    siv.add_global_callback(Event::CtrlChar('q'), on_quit);
    siv.add_global_callback(Event::CtrlChar('h'), on_help);

    siv.menubar()
        .add_subtree(
            constants::labels::FILE,
            MenuTree::new()
                .leaf("Open...", menu::open)
                .leaf("Close", menu::close)
                .leaf("Save Progress", |_| {}) // TODO
                .delimiter()
                .leaf(constants::labels::QUIT, on_quit),
        )
        .add_delimiter()
        .add_leaf(constants::labels::HELP, on_help)
        .add_delimiter()
        .add_leaf(constants::labels::QUIT, on_quit);
    siv.set_autohide_menu(false);

    // DEBUG
    menu::load_storygame(
        &mut siv,
        std::path::Path::new("./examples/yaml/Storygame.yaml"),
    );

    siv.add_layer(DummyView);
    redraw_content(&mut siv);

    siv.run();
}

/*
 * Rendering.
 */

lazy_static! {
    static ref FILLER_TEXT: &'static str =
        Box::leak(iter::repeat('~').take(9999).collect::<String>().into());
}

fn redraw_content(siv: &mut Cursive) {
    // Content view - container for main content.
    fn content_view(s: &mut Cursive) -> impl View {
        let (title, content) = s
            .with_user_data(|app: &mut AppState| {
                app.game.as_ref().map(|game| {
                    let page = game.current_page.borrow();
                    let content = interpolate(&page.content, &game);
                    (page.title.clone(), content)
                })
            })
            .flatten()
            .unwrap_or_else(|| (None, StyledString::plain(*FILLER_TEXT)));

        OnEventView::new({
            let dialog = Dialog::around(TextView::new(content).scrollable().with_name("content"))
                .h_align(HAlign::Center)
                .padding_lrtb(0, 0, 0, 1)
                .button(constants::labels::NEXT, on_continue);
            match title {
                Some(title) => dialog.title(format!("\"{}\"", title)),
                None => dialog,
            }
        })
        .on_event(Event::CtrlChar('n'), on_continue)
        .on_event('k', mk_scroll("content", |_| -1))
        .on_event('j', mk_scroll("content", |_| 1))
        .on_event(
            Event::CtrlChar('u'),
            mk_scroll("content", |r: Rect| -(r.height() as i32) / 2),
        )
        .on_event(
            Event::CtrlChar('d'),
            mk_scroll("content", |r: Rect| r.height() as i32 / 2),
        )
        .on_event(' ', mk_scroll("content", |r: Rect| r.height() as i32))
        .on_event('g', |s: &mut Cursive| {
            s.call_on_name("content", |view: &mut ScrollView<TextView>| {
                view.get_scroller_mut().scroll_to_top();
            });
        })
        .on_event('G', |s: &mut Cursive| {
            s.call_on_name("content", |view: &mut ScrollView<TextView>| {
                view.get_scroller_mut().scroll_to_bottom();
            });
        })
    }

    // Debug panel - container for Variables view and Logs view.
    #[cfg(debug_assertions)]
    fn debug_panel(s: &mut Cursive) -> impl View {
        let vars_view = s
            .with_user_data(|app: &mut AppState| {
                app.game.as_ref().map(|game: &Game| {
                    let mut vars = game.variables.iter().collect::<Vec<_>>();
                    vars.sort_by(|x, y| x.0.cmp(y.0));

                    let mut view = ListView::new();
                    for (name, value) in vars {
                        view.add_child(
                            &format!("> {}", name),
                            TextView::new({
                                let mut s = StyledString::plain("::  ");
                                s.append(StyledString::styled(
                                    value.to_string(),
                                    Style::from(Color::Dark(BaseColor::Blue)).combine(Effect::Bold),
                                ));
                                s
                            }),
                        );
                    }
                    view
                })
            })
            .flatten()
            .unwrap_or_else(|| ListView::new().child("ERROR", TextView::new("N/A")));

        LinearLayout::vertical()
            .child(
                // Variables view - lists the current values of each variable.
                Panel::new(ScrollView::new(vars_view).scroll_x(true))
                    .title("VARS")
                    .title_position(HAlign::Right),
            )
            .child(
                // Logs view - displays logging statements.
                Panel::new(ScrollView::new(LogView).scroll_x(true))
                    .title("LOGS")
                    .title_position(HAlign::Right)
                    .full_height(),
            )
    }

    siv.pop_layer();

    // If there are Prompts in the queue, display the next dialog.
    if let Some(dialog) = pop_prompt_dialog(siv) {
        siv.add_layer(dialog);
    // Otherwise, display the main layout.
    } else {
        let mut layout = LinearLayout::horizontal().child(content_view(siv));
        #[cfg(debug_assertions)]
        {
            layout.add_child(debug_panel(siv));
        }
        siv.add_fullscreen_layer(layout);
    }
}

fn pop_prompt_dialog(siv: &mut Cursive) -> Option<impl View> {
    siv.with_user_data(|app: &mut AppState| {
        let game = app.game.as_mut().unwrap();

        game.pop_prompt().map(|Prompt { text, variable }| {
            let content = interpolate(&text, &game);

            match variable {
                // Prompt has a `variable`, so create an input dialog.
                Some(var_name) => {
                    let var_name_clone = var_name.clone();
                    Dialog::around(
                        LinearLayout::vertical()
                            .child(PaddedView::new(
                                Margins::lrtb(1, 1, 1, 1),
                                TextView::new(content),
                            ))
                            .child(Panel::new(
                                EditView::new()
                                    .on_submit(move |s: &mut Cursive, input: &str| {
                                        on_prompt_submit(s, input, &var_name);
                                    })
                                    .with_name("prompt-input"),
                            )),
                    )
                    .button("Ok", move |s: &mut Cursive| {
                        let input = s
                            .call_on_name("prompt-input", |view: &mut EditView| view.get_content())
                            .unwrap();
                        on_prompt_submit(s, input.as_ref(), &var_name_clone);
                    })
                    .title("PROMPT")
                }
                // Prompt has no `variable`, so create a simple info dialog.
                None => Dialog::text(content)
                    .button("Ok", |s: &mut Cursive| {
                        s.pop_layer();
                        redraw_content(s);
                    })
                    .title("INFO"),
            }
        })
    })
    .unwrap()
}

fn on_prompt_submit(siv: &mut Cursive, input: &str, var_name: &String) {
    let value: Variable = match input.parse() {
        Ok(value) => value,
        Err(err) => {
            siv.add_layer(Dialog::info(format!("Invalid value: {}.", err)));
            return;
        }
    };

    let maybe_err = if input.is_empty() {
        Err("Input must not be empty.".to_string())
    } else {
        siv.with_user_data(|app: &mut AppState| {
            let variable = app
                .game
                .as_mut()
                .unwrap()
                .variables
                .get_mut(var_name)
                .unwrap();
            if variable.type_() != value.type_() {
                return Err(format!("Please enter a {}.", variable.type_()));
            }

            *variable = value;
            Ok(())
        })
        .unwrap()
    };

    match maybe_err {
        Ok(_) => {
            siv.pop_layer();
            redraw_content(siv);
        }
        Err(msg) => siv.add_layer(Dialog::info(msg)),
    };
}

fn mk_scroll<F>(view_name: &'static str, f: F) -> impl Fn(&mut Cursive)
where
    F: Fn(Rect) -> i32,
{
    move |s: &mut Cursive| {
        s.call_on_name(view_name, |view: &mut ScrollView<TextView>| {
            let viewport = view.content_viewport();
            let delta = f(viewport);
            view.get_scroller_mut().scroll_to_y(if delta.is_negative() {
                viewport.top().saturating_sub(-delta as usize)
            } else {
                viewport.bottom() + (delta as usize)
            });
        })
        .unwrap();
    }
}

fn interpolate(content: &str, game: &Game) -> StyledString {
    let reg = Handlebars::new();
    let content = match reg.render_template(content, &game.variables) {
        Ok(content) => content,
        Err(err) => {
            error!("error rendering template: {}", err);
            content.to_owned()
        }
    };
    markdown::parse(content)
}

/*
 * Event handling.
 */

fn on_continue(siv: &mut Cursive) {
    let current_page = match siv
        .with_user_data(|app: &mut AppState| {
            app.game.as_ref().map(|game| Rc::clone(&game.current_page))
        })
        .flatten()
    {
        Some(page) => page,
        None => return,
    };
    let prompt = &current_page.borrow().prompt;
    let links = &current_page.borrow().links;

    let mut select = SelectView::<usize>::new().on_submit(|s: &mut Cursive, link_idx: &usize| {
        s.pop_layer();

        let game_over = s
            .with_user_data(|app: &mut AppState| {
                let game = app.game.as_mut().unwrap();
                game.follow_link(*link_idx)
                    .map(|msg| interpolate(&msg, &game))
            })
            .flatten();

        if let Some(msg) = game_over {
            s.add_layer(
                OnEventView::new(
                    Dialog::around(TextView::new(msg).h_align(HAlign::Center))
                        .h_align(HAlign::Center)
                        .button("OK", |s| s.quit())
                        .button("Cancel", on_menu_back),
                )
                .on_event(Event::CtrlChar('b'), on_menu_back),
            );
        } else {
            redraw_content(s);
        }
    });

    siv.with_user_data(|app: &mut AppState| {
        let game = app.game.as_mut().unwrap();
        for (idx, link) in links.iter().enumerate() {
            let mut sstr = StyledString::from("> ");
            sstr.append(interpolate(&link.text, &game));
            sstr.append_styled(format!("  ↪ ({}) ", link.dest), Effect::Italic);
            select.add_item(sstr, idx);
        }
    });

    siv.add_layer(
        OnEventView::new(
            Dialog::around(
                LinearLayout::vertical()
                    .child(TextView::new(
                        prompt.as_deref().unwrap_or("Choose an option."),
                    ))
                    .child(select.scrollable()),
            )
            .h_align(HAlign::Center)
            .button(constants::labels::BACK, on_menu_back),
        )
        .on_event(Event::CtrlChar('b'), on_menu_back),
    );
}

fn on_help(siv: &mut Cursive) {
    fn mk_help_section(title: &str, commands: &[(&str, &str)]) -> Panel<LinearLayout> {
        let mut layout = LinearLayout::vertical();
        for (text, key) in commands {
            layout.add_child(
                LinearLayout::horizontal()
                    .child(
                        TextView::new(text.to_string())
                            .h_align(HAlign::Left)
                            .full_width(),
                    )
                    .child(TextView::new(key.to_string()).h_align(HAlign::Right)),
            )
        }
        Panel::new(layout).title(title).title_position(HAlign::Left)
    }

    siv.add_layer(
        Dialog::around(
            Panel::new(
                LinearLayout::vertical()
                    .child(mk_help_section(
                        "General",
                        constants::help::commands::GENERAL,
                    ))
                    .child(mk_help_section(
                        "Story Navigation",
                        constants::help::commands::NAVIGATION,
                    )),
            )
            .title("Help")
            .scrollable(),
        )
        .h_align(HAlign::Center)
        .button("Done", on_menu_back)
        .max_width(((siv.screen_size().x as f32 * 0.75).round() as usize).min(50)),
    );
}

fn on_quit(siv: &mut Cursive) {
    siv.add_layer(
        OnEventView::new(
            Dialog::text("Are you sure you want to quit?")
                .h_align(HAlign::Center)
                .button("OK", |s| s.quit())
                .button("Cancel", on_menu_back),
        )
        .on_event(Event::CtrlChar('q'), |s| s.quit()),
    )
}

fn on_menu_back(siv: &mut Cursive) {
    siv.pop_layer();
}
