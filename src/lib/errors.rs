// use std::backtrace::Backtrace;
use std::cell::{BorrowError, BorrowMutError};
use std::error::Error as StdError;
use std::path::{Path, PathBuf};
use std::{fmt, io};

use nonempty::NonEmpty;

use crate::types::{PageID, VarType, Variable};

pub type Result<T> = std::result::Result<T, Error>;

/// Inner struct for `Error::Internal`.
///
/// Each variant represents a type of programmer error.
#[derive(Debug)]
pub enum InternalError {
    Msg(String),
    RcGetMut,
    CellBorrow(BorrowError),
    CellBorrowMut(BorrowMutError),
    PathAttr(&'static str),
    Logger(Box<dyn StdError>),
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Inner type for `Error::ReadError`.
///
/// Each variant represents the type of file where the `ReadError` originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doctype {
    /// The settings file (e.g. 'Storygame.yaml').
    Settings,
    /// A story file.
    Story,
}

impl fmt::Display for Doctype {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Doctype::Settings => write!(f, "Settings"),
            Doctype::Story => write!(f, "Story"),
        }
    }
}

/// Crate error type.
#[derive(Debug)]
pub enum Error {
    Message(String),
    Expected(String),
    Unexpected(String),
    /// Invalid Page ID used in a story file.
    UndeclaredPageID(PageID),
    /// Undeclared variable used in story file.
    UndeclaredVariable(String),
    /// Wrong value type used in story file.
    BadValueType {
        value: Variable,
        expected: VarType,
    },
    /// Wrong variable type used in story file.
    BadVariableType {
        var_name: String,
        var_type: VarType,
        expected: VarType,
    },
    /// Error reading a file.
    ReadError {
        doctype: Doctype,
        path: PathBuf,
    },
    /// Parsing failed.
    ParseError {
        doctype: Doctype,
        path: PathBuf,
        error: serde_yaml::Error,
    },
    /// Reading TOML failed.
    Deserialize(serde_yaml::Error),
    /// Programmer error.
    Internal {
        error: InternalError,
        // backtrace: Backtrace,
    },
    /// IO error.
    IO(io::Error),
    /// Wrapper for any [`std::error::Error`].
    Std(Box<dyn StdError>),
    /// Multiple errors.
    Errors(NonEmpty<Box<Error>>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.to_string_vec(false).join(": ").as_str())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::ParseError { error, .. } => Some(error),
            Error::Deserialize(e) => Some(e),
            Error::Internal { error } => match error {
                InternalError::CellBorrow(e) => Some(e),
                InternalError::CellBorrowMut(e) => Some(e),
                InternalError::Logger(e) => Some(e.as_ref()),
                _ => None,
            },
            Error::IO(e) => Some(e),
            Error::Std(e) => Some(e.as_ref()),
            Error::Errors(es) => Some(es.head.as_ref()),
            _ => None,
        }
    }
}

macro_rules! impl_From_for_Error_Internal {
    ($(From<$from:ty> $to_internal_err:expr);* $(;)*) => {
        $(
            impl From<$from> for InternalError {
                fn from(e: $from) -> Self {
                    $to_internal_err(e)
                }
            }

            impl From<$from> for Error {
                fn from(e: $from) -> Self {
                    Error::Internal { error: e.into() }
                }
            }
        )*
    };
}

impl_From_for_Error_Internal! {
    From<BorrowError> |e| InternalError::CellBorrow(e);
    From<BorrowMutError> |e| InternalError::CellBorrowMut(e);
    From<log4rs::config::Errors> |e| InternalError::Logger(Box::new(e));
    From<log::SetLoggerError> |e| InternalError::Logger(Box::new(e));
}

impl From<serde_yaml::Error> for Error {
    fn from(e: serde_yaml::Error) -> Self {
        Error::Deserialize(e)
    }
}
impl From<InternalError> for Error {
    fn from(error: InternalError) -> Self {
        Error::Internal { error }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e)
    }
}
impl<E: StdError + 'static> From<Box<E>> for Error {
    fn from(e: Box<E>) -> Self {
        Error::Std(e)
    }
}

impl Error {
    /// Constructor method for [`Error::Message`].
    pub fn message<S: ToString>(s: S) -> Self {
        Error::Message(s.to_string())
    }
    /// Constructor method for [`Error::Expected`].
    pub fn expected<S: ToString>(s: S) -> Self {
        Error::Expected(s.to_string())
    }
    /// Constructor method for [`Error::Unexpected`].
    pub fn unexpected<S: ToString>(s: S) -> Self {
        Error::Unexpected(s.to_string())
    }
    /// Constructor method for [`Error::UndeclaredPageID`].
    pub fn undeclared_page_id<S: ToString>(s: S) -> Self {
        Error::UndeclaredPageID(s.to_string())
    }
    /// Constructor method for [`Error::UndeclaredVariable`].
    pub fn undeclared_variable<S: ToString>(s: S) -> Self {
        Error::UndeclaredVariable(s.to_string())
    }
    pub fn bad_value_type(value: &Variable, expected: VarType) -> Self {
        Error::BadValueType {
            value: value.clone(),
            expected,
        }
    }
    pub fn bad_variable_type<S: ToString>(
        var_name: S,
        var_type: VarType,
        expected: VarType,
    ) -> Self {
        Error::BadVariableType {
            var_name: var_name.to_string(),
            var_type,
            expected,
        }
    }
    /// Constructor method for [`Error::ReadError`].
    pub fn read_error<P: AsRef<Path>>(doctype: Doctype, path: P) -> Self {
        Error::ReadError {
            doctype,
            path: path.as_ref().to_path_buf(),
        }
    }
    /// Constructor method for [`Error::ParseError`].
    pub fn parse_error<P: AsRef<Path>>(
        doctype: Doctype,
        path: P,
        error: serde_yaml::Error,
    ) -> Self {
        Error::ParseError {
            doctype,
            path: path.as_ref().to_path_buf(),
            error,
        }
    }
    /// Constructor method for [`Error::Internal`] with [`InternalError::Logger`].
    pub fn logger<E: StdError + 'static>(e: E) -> Self {
        Error::Internal {
            error: InternalError::Logger(Box::new(e)),
        }
    }
    /// Constructor method for [`Error::Std`].
    pub fn std<E: StdError + 'static>(e: E) -> Self {
        Error::Std(Box::new(e))
    }
    /// Constructor method for [`Error::Errors`].
    ///
    /// Returns [`None`](Option::None) if the `errors` has no elements.
    pub fn errors<I>(errors: I) -> Option<Self>
    where
        I: IntoIterator,
        I::Item: Into<Error>,
    {
        Some(Error::Errors(NonEmpty::from_vec(
            errors.into_iter().map(|e| Box::new(e.into())).collect(),
        )?))
    }

    /// Join this Error and another into an [`Error::Errors`].
    pub fn join<E: Into<Error>>(self, other: E) -> Error {
        Error::errors(vec![self, other.into()]).unwrap()
    }

    /// Formats the Error as a verbose String.
    ///
    /// Inner Errors in Compound Error variants are terminated in periods and separated by two
    /// newlines. Other Error variants are each formatted as a single line with `to_string`.
    pub fn to_string_verbose(&self) -> String {
        let parts: Vec<String> = self
            .to_string_vec(true)
            .into_iter()
            .filter_map(|s: String| match s.trim() {
                "" => None,
                s => {
                    let (head, tail) = s.split_at(1);
                    Some(String::from(head.to_uppercase()) + tail)
                }
            })
            .collect();
        match self {
            Error::Errors(_) => parts.join(".\n\n").trim_end_matches(".").to_owned() + ".",
            _ => parts.join(": "),
        }
    }

    /// Generates a vector of String "parts" which can be errors together (e.g. with a delimiter) to
    /// form a String representation.
    ///
    /// Used internally by the [`Display`](fmt::Display) implementation and
    /// [`to_string_verbose`](#method.to_string_verbose).
    ///
    /// * `verbose` - Whether to format nested Errors verbosely (for compound Error variants).
    pub fn to_string_vec(&self, verbose: bool) -> Vec<String> {
        let to_s: fn(&Error) -> String = if verbose {
            |e| e.to_string_verbose()
        } else {
            |e| e.to_string()
        };

        match self {
            Error::Message(s) => vec![s.clone()],
            Error::Expected(s) => vec![format!("expected {}", s)],
            Error::Unexpected(s) => vec![format!("unexpected {}", s)],
            Error::UndeclaredPageID(id) => vec![
                "invalid page ID".to_string(),
                format!("no page exists with ID '{}'", id),
            ],
            Error::UndeclaredVariable(name) => vec![format!("undeclared variable '{}'", name)],
            Error::BadValueType { value, expected } => vec![
                format!("bad type for value {:?}", value),
                format!("expected a {}", expected),
            ],
            Error::BadVariableType {
                var_name,
                var_type,
                expected,
            } => vec![
                format!("variable `{}` has wrong type", var_name),
                format!("expected a {}, but got a {}", var_type, expected),
            ],
            Error::ReadError { doctype, path } => vec![format!(
                "could not read {} file at `{}`",
                doctype,
                path.display()
            )],
            Error::ParseError {
                doctype,
                path,
                error,
            } => {
                let context = match error.location() {
                    Some(location) => format!(
                        "{}:{}:{}",
                        path.display(),
                        location.line(),
                        location.column()
                    ),
                    None => path.display().to_string(),
                };
                vec![
                    format!("failed to parse {} file at `{}`", doctype, context),
                    error.to_string(),
                ]
            }
            Error::Deserialize(e) => vec!["could not parse TOML".to_string(), e.to_string()],
            Error::Internal { error } => vec!["internal error".to_string(), error.to_string()],
            Error::IO(e) => vec!["I/O failure".to_string(), e.to_string()],
            Error::Std(e) => vec![e.to_string()],
            Error::Errors(es) => es.iter().map(|e| to_s(e.as_ref())).collect(),
        }
    }
}
