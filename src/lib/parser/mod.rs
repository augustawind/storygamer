mod settings;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use either::Either::*;
use regex::Regex;
use same_file::is_same_file;

pub use self::settings::Settings;
use crate::errors::{Doctype, Error, InternalError, Result};
use crate::types::{
    ComparisonOp, Condition, ItemDef, LinkAction, LinkDest, Operation, Page, PageID, Prompt,
    VarType, Variable,
};

lazy_static! {
    static ref RE_DOCUMENT_SEP: Regex =
        Regex::new(r#"(?xm) (\A | (^ \.{3} .* $)?) (^ -{3} \s* $) | \A"#).unwrap();
}

/// Reads and parses a storygame using the given [`Settings`].
///
/// 1. Reads files from [`Settings.base_dir`].
/// 2. Parses file contents into [`Page`] objects.
/// 3. Validates and finalizes parsed data.
/// 4. Returns the [`Page`] which is designated as the entrypoint.
pub fn parse(settings: &Settings) -> Result<Rc<RefCell<Page>>> {
    let mut pages = read_pages(settings)?;
    let pages_clone = pages.clone();

    let page_ids = settings.pages();
    let variables = settings.variables();
    let items = settings.items();

    for page_id in pages_clone.keys() {
        // Check that page IDs are declared in settings.
        if !page_ids.contains(page_id) {
            return Err(Error::undeclared_page_id(page_id));
        }
        let page = pages
            .get_mut(page_id)
            .ok_or_else(|| Error::undeclared_page_id(page_id))?;

        /*
         * Define `clean_*` functions.
         */

        let clean_link_dest = |dest: &mut LinkDest| -> Result<()> {
            if let LinkDest::Page(ref mut to_page) = dest {
                if let Left(ref to_page_id) = to_page {
                    let child = Rc::clone(
                        pages_clone
                            .get(to_page_id)
                            .ok_or_else(|| Error::undeclared_page_id(to_page_id))?,
                    );
                    if let Ok(mut child_ref) = child.try_borrow_mut() {
                        child_ref.parents.push(Rc::downgrade(&page));
                    }
                    *to_page = Right(child);
                }
            }
            Ok(())
        };

        let clean_action = |action: &mut LinkAction| -> Result<()> {
            match action {
                // Check that variables are declared in settings and that values have correct types.
                LinkAction::SetVar { name, value } => match variables.get(name) {
                    Some(var) if var.type_eq(value) => {}
                    Some(var) => {
                        return Err(Error::bad_value_type(value, var.type_()));
                    }
                    None => return Err(Error::undeclared_variable(name)),
                },
                LinkAction::ModNum { name, .. } => match variables.get(name) {
                    Some(Variable::Num(_)) => {}
                    Some(var) => {
                        return Err(Error::bad_variable_type(name, var.type_(), VarType::Num))
                    }
                    None => return Err(Error::undeclared_variable(name)),
                },
                LinkAction::ToggleBool(name) => match variables.get(name) {
                    Some(Variable::Bool(_)) => {}
                    Some(var) => {
                        return Err(Error::bad_variable_type(name, var.type_(), VarType::Bool))
                    }
                    None => return Err(Error::undeclared_variable(name)),
                },
                &mut LinkAction::SetDest(ref mut dest) => {
                    clean_link_dest(dest)?;
                }
                LinkAction::Prompt(Prompt { variable, .. }) => {
                    if let Some(var_name) = variable {
                        if !variables.contains_key(var_name.as_str()) {
                            return Err(Error::undeclared_variable(var_name));
                        }
                    }
                }
                LinkAction::AcquireItem(name)
                | LinkAction::DropItem(name)
                | LinkAction::UseItem(name) => {
                    if !items.contains_key(name) {
                        return Err(Error::undeclared_item(name));
                    }
                }
            }
            Ok(())
        };

        fn clean_operation(
            operation: &mut Operation,
            variables: &HashMap<String, Variable>,
        ) -> Result<()> {
            let var_name = &operation.name;
            let var = variables
                .get(var_name)
                .ok_or_else(|| Error::undeclared_variable(var_name))?;

            use ComparisonOp::*;
            match operation.op {
                GT | GTE | LT | LTE => {
                    if var.type_() != VarType::Num {
                        return Err(Error::bad_variable_type(
                            var_name,
                            var.type_(),
                            VarType::Num,
                        ));
                    }
                    if operation.value.type_() != VarType::Num {
                        return Err(Error::bad_value_type(&operation.value, VarType::Num));
                    }
                }
                _ => {}
            };
            Ok(())
        }

        fn clean_condition(
            cond: &mut Condition,
            variables: &HashMap<String, Variable>,
            items: &HashMap<String, ItemDef>,
        ) -> Result<()> {
            match cond {
                Condition::And(children) | Condition::Or(children) => {
                    for child in children.iter_mut() {
                        clean_condition(child, variables, items)?;
                    }
                }
                Condition::Op(operation) => {
                    clean_operation(operation, variables)?;
                }
                Condition::Not(condition) => {
                    clean_condition(condition, variables, items)?;
                }
                Condition::HasItem(name) => {
                    if !items.contains_key(name) {
                        return Err(Error::undeclared_item(name));
                    }
                }
            }
            Ok(())
        }

        /*
         * Loop through pages and run `clean_*` functions on them.
         */

        for link in &mut page.borrow_mut().links.iter_mut() {
            clean_link_dest(&mut link.dest)?;

            for trigger in link.triggers.iter_mut() {
                clean_condition(&mut trigger.condition, variables, items)?;
                for action in trigger.actions.iter_mut() {
                    clean_action(action)?;
                }
            }
            for action in link.actions.iter_mut() {
                clean_action(action)?;
            }
        }
    }

    // Return entrypoint page.
    Ok(pages
        .remove(
            &settings
                .entrypoint()
                .file_stem()
                .ok_or_else(|| InternalError::PathAttr("file_stem"))?
                .to_str()
                .unwrap()
                .to_owned(),
        )
        .unwrap())
}

fn read_pages(settings: &Settings) -> Result<HashMap<String, Rc<RefCell<Page>>>> {
    let config_path = settings.source();

    // Read content from all source files.
    let mut sources = Vec::new();
    for entry in fs::read_dir(settings.base_dir())? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        if let Some(cfg_path) = config_path {
            if let Ok(true) = is_same_file(&path, cfg_path) {
                continue;
            }
        }
        let content = fs::read_to_string(&path)?;
        sources.push((path, content));
    }

    // Parse content into one or more pages from each source file.
    let parsed_pages = sources
        .iter()
        .flat_map(|(path, content)| {
            RE_DOCUMENT_SEP.split(content).filter_map(move |s| {
                if s.trim().is_empty() {
                    return None;
                }
                Some(
                    serde_yaml::from_str::<Page>(s)
                        .map_err(|e| Error::parse_error(Doctype::Story, path, e)),
                )
            })
        })
        .collect::<Result<Vec<Page>>>()?;

    // Convert pages Vec to a HashMap.
    let pages = parsed_pages
        .into_iter()
        .map(|p: Page| (p.id.to_owned(), Rc::new(RefCell::new(p))))
        .collect();

    Ok(pages)
}
