use super::State;
use crate::providers::delegate::Delegation;
use std::collections::{HashMap, VecDeque};

/// Whether two file lists touch the same file (a listed folder covers the files inside it).
fn overlap(a: &[String], b: &[String]) -> bool {
    let clean = |p: &String| p.replace('\\', "/").trim_start_matches("./").trim_end_matches('/').to_lowercase();
    a.iter().map(clean).any(|x| b.iter().map(clean).any(|y| x == y || x.starts_with(&format!("{y}/")) || y.starts_with(&format!("{x}/"))))
}

pub(super) fn key(title: &str) -> String {
    title.trim().to_lowercase()
}

pub(super) enum Next {
    Run(Delegation),
    Skip(Delegation, String),
    Wait,
}

/// The first queued task whose dependencies are settled. Independent tasks never wait for a check.
pub(super) fn next_ready(queue: &mut VecDeque<Delegation>, state: &HashMap<String, State>) -> Next {
    let queued: Vec<String> = queue.iter().map(|t| key(&t.title)).collect();
    for index in 0..queue.len() {
        let own = key(&queue[index].title);
        let (mut blocked, mut waiting) = (None, false);
        for dep in queue[index].depends_on.iter().map(|d| key(d)).filter(|d| *d != own) {
            match state.get(&dep) {
                Some(State::Failed) => blocked = Some(dep),
                Some(State::Checking) => waiting = true,
                Some(State::Clean) => {}
                None => waiting |= queued.contains(&dep),
            }
        }
        if let Some(dep) = blocked {
            return Next::Skip(queue.remove(index).expect("index is in range"), format!("it depends on \"{dep}\", which did not pass"));
        }
        if !waiting {
            return Next::Run(queue.remove(index).expect("index is in range"));
        }
    }
    Next::Wait
}

/// A queued task that may run beside `first`: its dependencies are settled (`first` itself is not), it is not a big task, and it
/// shares no file with `first`. Taken out of the queue.
pub(super) fn take_partner(queue: &mut VecDeque<Delegation>, state: &HashMap<String, State>, first: &Delegation, skip: impl Fn(&Delegation) -> bool) -> Option<Delegation> {
    let queued: Vec<String> = queue.iter().map(|t| key(&t.title)).collect();
    let index = queue.iter().position(|task| {
        let own = key(&task.title);
        let settled = task.depends_on.iter().map(|d| key(d)).filter(|d| *d != own).all(|dep| match state.get(&dep) {
            Some(State::Clean) => true,
            None => dep != key(&first.title) && !queued.contains(&dep),
            _ => false,
        });
        settled && !skip(task) && !task.files.is_empty() && !first.files.is_empty() && !overlap(&task.files, &first.files)
    })?;
    queue.remove(index)
}
