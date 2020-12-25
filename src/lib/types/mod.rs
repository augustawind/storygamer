mod condition;
mod variable;

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

use either::{Either, Either::*};
use serde::de::{Deserializer, Visitor};
use serde::Deserialize;

pub use self::condition::*;
pub use self::variable::*;

pub type PageID = String;

/// A page in a story.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: PageID,
    pub title: Option<String>,
    pub content: String,
    pub prompt: Option<String>,
    pub links: Vec<Link>,
    #[serde(skip)]
    pub parents: Vec<Weak<RefCell<Page>>>,
}

impl Page {
    pub fn new<S: Into<String>>(
        id: PageID,
        title: Option<String>,
        content: S,
        prompt: Option<String>,
        links: Vec<Link>,
    ) -> Self {
        Page {
            id,
            title,
            content: content.into(),
            prompt,
            links,
            parents: Vec::new(),
        }
    }
}

/// A link to somewhere else in the story, plus any associated actions.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub text: String,
    pub dest: LinkDest,
    #[serde(default)]
    pub triggers: Vec<LinkTrigger>,
    #[serde(default)]
    pub actions: Vec<LinkAction>,
}

/// The destination of a link.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub enum LinkDest {
    #[serde(rename = "page")]
    #[serde(deserialize_with = "deserialize_link_dest_page")]
    Page(Either<PageID, Rc<RefCell<Page>>>),
    #[serde(rename = "current")]
    CurrentPage,
    #[serde(rename = "previous")]
    PrevPage,
    #[serde(rename = "end")]
    EndGame(String),
}

fn deserialize_link_dest_page<'de, D>(
    deserializer: D,
) -> Result<Either<PageID, Rc<RefCell<Page>>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct LinkDestPageVisitor;

    impl<'de> Visitor<'de> for LinkDestPageVisitor {
        type Value = Either<PageID, Rc<RefCell<Page>>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("string containing a page ID")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
            Ok(Left(value.to_owned()))
        }
    }

    deserializer.deserialize_string(LinkDestPageVisitor)
}

impl fmt::Display for LinkDest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LinkDest::Page(to_page) => match to_page {
                Left(page_id) => write!(f, "#{}", page_id),
                Right(page) => write!(f, "#{}", page.borrow().id),
            },
            LinkDest::CurrentPage => f.write_str("<current page>"),
            LinkDest::PrevPage => f.write_str("<previous page>"),
            LinkDest::EndGame(_) => f.write_str("<end game>"),
        }
    }
}

impl LinkDest {
    pub fn get_page(&self) -> Option<Rc<RefCell<Page>>> {
        if let LinkDest::Page(maybe_page) = self {
            return Some(Rc::clone(
                maybe_page
                    .as_ref()
                    .expect_right("Left variant should only exist during parsing"),
            ));
        }
        None
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LinkTrigger {
    pub condition: Condition,
    pub actions: Vec<LinkAction>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub enum LinkAction {
    #[serde(rename = "set-var")]
    SetVar { name: String, value: Variable },
    #[serde(rename = "mod-num")]
    ModNum { name: String, value: i32 },
    #[serde(rename = "toggle-bool")]
    ToggleBool { name: String },
    #[serde(rename = "set-dest")]
    SetDest { dest: LinkDest },
    #[serde(rename = "prompt")]
    Prompt(Prompt),
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Prompt {
    pub text: String,
    #[serde(default)]
    pub variable: Option<String>,
}
