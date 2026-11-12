//! `core.md`: the only memory text injected at a new chat. Rebuilt by the app, never by the AI.
use super::session::{line, Session};

pub const CORE_MAX: usize = 1600;
/// Open tasks may use at most this much, so the session list always fits.
const OPEN_MAX: usize = 900;

/// `sessions` must be newest first.
pub fn build(sessions: &[Session], limit: usize) -> String {
    if sessions.is_empty() {
        return String::new();
    }
    let mut out = String::from("Project memory (earlier AI notes; verify against code/git):\n");
    let open: Vec<String> = sessions
        .iter()
        .filter(|s| !s.retired())
        .flat_map(|s| s.open.iter().map(|t| format!("- [{}] {}\n", s.id, line(t))))
        .collect();
    if !open.is_empty() {
        out.push_str("Open tasks:\n");
        let mut used = 0;
        for (i, task) in open.iter().enumerate() {
            used += task.chars().count();
            if used > OPEN_MAX {
                out.push_str(&format!("- (+{} more)\n", open.len() - i));
                break;
            }
            out.push_str(task);
        }
    }
    out.push_str("Sessions:\n");
    for s in sessions.iter().take(limit) {
        let title = if s.title.is_empty() { &s.id } else { &s.title };
        out.push_str(&format!("- {} · {} · {}\n", line(title), s.status, line(&s.goal)));
    }
    out.chars().take(CORE_MAX).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(id: &str, status: &str, open: &[&str]) -> Session {
        Session {
            id: id.into(),
            title: format!("T-{id}"),
            status: status.into(),
            goal: format!("G-{id}"),
            open: open.iter().map(|x| x.to_string()).collect(),
            ..Default::default()
        }
    }
    #[test]
    fn open_tasks_first_then_limited_sessions_and_retired_only_one_line() {
        let all = [s("a", "active", &["do x"]), s("b", "done", &["stale"]), s("c", "blocked", &[])];
        let t = build(&all, 2);
        assert!(t.find("[a] do x").unwrap() < t.find("Sessions:").unwrap());
        assert!(t.contains("T-a · active · G-a") && t.contains("T-b · done · G-b"));
        assert!(!t.contains("stale") && !t.contains("T-c"));
    }
    #[test]
    fn hard_capped() {
        let mut many: Vec<Session> = (0..50).map(|i| s(&format!("c{i}"), "active", &["x".repeat(80).as_str()])).collect();
        many.iter_mut().for_each(|x| x.goal = "g".repeat(300));
        assert_eq!(build(&many, 10).chars().count(), CORE_MAX);
        assert!(build(&[], 3).is_empty());
    }
    #[test]
    fn many_open_tasks_leave_room_for_sessions() {
        let many: Vec<Session> = (0..20).map(|i| s(&format!("c{i}"), "active", &["y".repeat(70).as_str()])).collect();
        let t = build(&many, 5);
        assert!(t.contains("more)") && t.contains("T-c4 · active · G-c4"));
    }
}
