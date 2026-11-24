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

/// Removes the first ready task independent of every task already in the group.
pub(super) fn take_partner(queue: &mut VecDeque<Delegation>, state: &HashMap<String, State>, group: &[Delegation]) -> Option<Delegation> {
    if group.iter().any(|task| task.files.is_empty()) {
        return None;
    }
    let grouped: Vec<_> = group.iter().map(|task| key(&task.title)).collect();
    let queued: Vec<String> = queue.iter().map(|t| key(&t.title)).collect();
    let index = queue.iter().position(|task| {
        let own = key(&task.title);
        if grouped.contains(&own) || task.files.is_empty() {
            return false;
        }
        let settled = task.depends_on.iter().map(|d| key(d)).filter(|d| *d != own).all(|dep| {
            !grouped.contains(&dep) && match state.get(&dep) {
                Some(State::Clean) => true,
                None => !queued.contains(&dep),
                _ => false,
            }
        });
        settled && group.iter().all(|other| {
            !other.depends_on.iter().any(|dep| key(dep) == own)
                && !overlap(&task.files, &other.files)
        })
    })?;
    queue.remove(index)
}
