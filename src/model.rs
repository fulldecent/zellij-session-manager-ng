//! Running sessions and saved sessions, and the find match.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Running,
    Exited,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub name: String,
    pub kind: Kind,
    pub current: bool,
    pub clients: usize,
    pub tabs: usize,
    pub age: Duration,
}

impl Session {
    pub fn can_kill(&self) -> bool {
        self.kind == Kind::Running && !self.current
    }

    pub fn can_delete(&self) -> bool {
        self.kind == Kind::Exited
    }
}

pub fn sort_sessions(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        kind_rank(a)
            .cmp(&kind_rank(b))
            .then_with(|| a.age.cmp(&b.age))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

fn kind_rank(session: &Session) -> u8 {
    if session.current {
        0
    } else if session.kind == Kind::Running {
        1
    } else {
        2
    }
}

pub fn age_label(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        "now".to_string()
    } else if secs < 3_600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3_600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

pub fn detail(session: &Session) -> String {
    match session.kind {
        Kind::Running if session.current => format!("current  {} tabs", session.tabs),
        Kind::Running => format!("{}  {} tabs", clients(session.clients), session.tabs),
        Kind::Exited => format!("exited  {}", age_label(session.age)),
    }
}

fn clients(count: usize) -> String {
    if count == 1 {
        "1 client".to_string()
    } else {
        format!("{count} clients")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub index: usize,
    pub hits: Vec<usize>,
}

pub fn filter(sessions: &[Session], query: &str) -> Vec<Found> {
    if query.is_empty() {
        return (0..sessions.len())
            .map(|index| Found {
                index,
                hits: Vec::new(),
            })
            .collect();
    }
    let mut scored = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        if let Some((score, hits)) = fuzzy(&session.name, query) {
            scored.push((score, index, hits));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .map(|(_, index, hits)| Found { index, hits })
        .collect()
}

fn fuzzy(name: &str, query: &str) -> Option<(i64, Vec<usize>)> {
    let name_chars: Vec<char> = name.chars().collect();
    let query_chars: Vec<char> = query.chars().flat_map(|ch| ch.to_lowercase()).collect();
    if query_chars.is_empty() {
        return Some((0, Vec::new()));
    }
    let mut hits = Vec::new();
    let mut query_index = 0;
    let mut score = 0i64;
    let mut previous = None;
    let mut started = None;
    for (index, ch) in name_chars.iter().enumerate() {
        if query_index >= query_chars.len() {
            break;
        }
        if ch.to_lowercase().next() == Some(query_chars[query_index]) {
            if started.is_none() {
                started = Some(index as i64);
            }
            score += if previous == Some(index.wrapping_sub(1)) {
                8
            } else {
                2
            };
            hits.push(index);
            previous = Some(index);
            query_index += 1;
        }
    }
    if query_index < query_chars.len() {
        return None;
    }
    score -= started.unwrap_or(0);
    Some((score, hits))
}

pub fn layout_label(info: &zellij_tile::prelude::LayoutInfo) -> String {
    use zellij_tile::prelude::LayoutInfo;
    match info {
        LayoutInfo::BuiltIn(name) | LayoutInfo::File(name, _) => name.clone(),
        LayoutInfo::Url(url) => url.rsplit('/').next().unwrap_or(url).to_string(),
        LayoutInfo::Stringified(_) => "raw".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(name: &str, current: bool, age_secs: u64) -> Session {
        Session {
            name: name.to_string(),
            kind: Kind::Running,
            current,
            clients: 1,
            tabs: 1,
            age: Duration::from_secs(age_secs),
        }
    }

    #[test]
    fn current_session_sorts_first() {
        let mut sessions = vec![
            running("other", false, 10),
            Session {
                name: "old".into(),
                kind: Kind::Exited,
                current: false,
                clients: 0,
                tabs: 0,
                age: Duration::from_secs(5),
            },
            running("here", true, 99),
        ];
        sort_sessions(&mut sessions);
        let names: Vec<_> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["here", "other", "old"]);
    }

    #[test]
    fn fuzzy_prefers_the_tighter_name() {
        let sessions = vec![running("work", false, 1), running("workspace", false, 2)];
        let found = filter(&sessions, "work");
        assert_eq!(sessions[found[0].index].name, "work");
    }
}
