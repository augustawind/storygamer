use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use crate::types::{
    ComparisonOp, Condition, LinkAction, LinkDest, LinkTrigger, Operation, Page, Prompt, Variable,
};
use crate::utils::ConvertBounded;

pub struct Game {
    pub starting_page: Rc<RefCell<Page>>,
    pub current_page: Rc<RefCell<Page>>,
    pub current_link_idx: Option<usize>,

    pub variables: HashMap<String, Variable>,
    pub history: Vec<HistoryItem>,
    pub prompt_queue: VecDeque<Prompt>,
}

impl Game {
    pub fn new(starting_page: &Rc<RefCell<Page>>, variables: &HashMap<String, Variable>) -> Self {
        Game {
            starting_page: Rc::clone(starting_page),
            current_page: Rc::clone(starting_page),
            current_link_idx: None,
            variables: variables.clone(),
            history: Vec::new(),
            prompt_queue: VecDeque::new(),
        }
    }

    pub fn pop_prompt(&mut self) -> Option<Prompt> {
        self.prompt_queue.pop_front()
    }

    /// Advance the Game by selecting the Link with the given `link_idx`.
    pub fn next(&mut self, link_idx: usize) -> Option<String> {
        trace!("next(idx={})", link_idx);

        let (mut link_dest, actions, triggers) = {
            let links = &self.current_page.borrow().links;
            let to_link = links.get(link_idx).unwrap();
            (
                to_link.dest.clone(),
                to_link.actions.clone(),
                to_link.triggers.clone(),
            )
        };

        if let Some(dest) = self.run_link_actions(actions) {
            link_dest = dest;
        }
        if let Some(dest) = self.eval_link_triggers(triggers) {
            link_dest = dest;
        }
        self.eval_link_dest(link_dest, link_idx)
    }

    /// Execute a series of [`LinkAction`](crate::types::LinkAction) in order for a given
    /// [`Page`](crate::types::Page). Each Action is executed with no awareness of previous Actions;
    /// therefore it is possible to override outcomes by adding an Action to the end.
    fn run_link_actions(&mut self, actions: Vec<LinkAction>) -> Option<LinkDest> {
        let mut final_dest = None;

        for action in actions {
            match action {
                LinkAction::SetVar { name, value } => {
                    if let Some(var) = self.variables.get_mut(&name) {
                        *var = value.clone();
                        debug!("action: set-var({}, {})", name, value);
                    }
                }
                LinkAction::ModNum { name, value } => {
                    if let Some(Variable::Num(var)) = self.variables.get_mut(&name) {
                        *var = i32::convert_bounded(*var as i32 + value as i32);
                        debug!("action: mod-num({}, {})", name, value);
                    }
                }
                LinkAction::ToggleBool { name } => {
                    if let Some(Variable::Bool(var)) = self.variables.get_mut(&name) {
                        *var = !*var;
                        debug!("action: toggle-bool({})", name);
                    }
                }
                LinkAction::SetDest { dest } => {
                    debug!("action: set-dest({})", dest);
                    final_dest = Some(dest);
                }
                LinkAction::Prompt(prompt) => {
                    debug!("action: prompt({:?})", prompt.variable);
                    self.prompt_queue.push_back(prompt);
                }
            }
        }

        final_dest
    }

    fn eval_link_triggers(&mut self, triggers: Vec<LinkTrigger>) -> Option<LinkDest> {
        let mut final_dest = None;

        for trigger in triggers {
            if self.eval_condition(&trigger.condition) {
                if let Some(dest) = self.run_link_actions(trigger.actions) {
                    final_dest = Some(dest);
                }
            }
        }

        final_dest
    }

    fn eval_link_dest(&mut self, link_dest: LinkDest, link_idx: usize) -> Option<String> {
        match link_dest {
            dest @ LinkDest::Page(_) => {
                let page = dest.get_page().unwrap();
                trace!("dest: page('{}')", page.borrow().id);

                if page.borrow().id != self.current_page.borrow().id {
                    self.history
                        .push(HistoryItem::new(&self.current_page, self.current_link_idx));
                    self.current_page = page;
                    self.current_link_idx = Some(link_idx);
                }
            }
            LinkDest::CurrentPage => {
                trace!("dest: current");
            }
            LinkDest::PrevPage => {
                trace!("dest: previous");
                if let Some(item) = self.history.pop() {
                    self.current_page = item.page.upgrade().unwrap();
                    self.current_link_idx = item.link_idx;
                }
            }
            LinkDest::EndGame(msg) => {
                trace!("dest: end");
                self.current_link_idx = None;
                return Some(msg);
            }
        }
        None
    }

    fn eval_condition(&mut self, cond: &Condition) -> bool {
        match cond {
            Condition::And(children) => children.iter().all(|child| self.eval_condition(child)),
            Condition::Or(children) => children.iter().any(|child| self.eval_condition(child)),
            Condition::Op(Operation { name, op, value }) => self.eval_operation(name, *op, value),
        }
    }

    fn eval_operation(&mut self, name: &String, op: ComparisonOp, value: &Variable) -> bool {
        use ComparisonOp::*;
        use Variable::*;
        let var = &self.variables[name];

        match op {
            EQ => match (var, value) {
                (Num(x), Num(y)) => x == y,
                (Bool(x), Bool(y)) => x == y,
                (Str(x), Str(y)) => x == y,
                _ => unreachable!(),
            },
            NEQ => match (var, value) {
                (Num(x), Num(y)) => x != y,
                (Bool(x), Bool(y)) => x != y,
                (Str(x), Str(y)) => x != y,
                _ => unreachable!(),
            },
            GT => match (var, value) {
                (Num(x), Num(y)) => x > y,
                _ => unreachable!(),
            },
            GTE => match (var, value) {
                (Num(x), Num(y)) => x >= y,
                _ => unreachable!(),
            },
            LT => match (var, value) {
                (Num(x), Num(y)) => x < y,
                _ => unreachable!(),
            },
            LTE => match (var, value) {
                (Num(x), Num(y)) => x <= y,
                _ => unreachable!(),
            },
        }
    }
}

pub struct HistoryItem {
    pub page: Weak<RefCell<Page>>,
    pub link_idx: Option<usize>,
}

impl HistoryItem {
    fn new(page: &Rc<RefCell<Page>>, link_idx: Option<usize>) -> Self {
        HistoryItem {
            page: Rc::downgrade(page),
            link_idx: link_idx,
        }
    }
}
