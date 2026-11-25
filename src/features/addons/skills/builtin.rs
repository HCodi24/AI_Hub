const BUILTIN: &[(&str, &str)] = &[("demo", include_str!("builtin/demo/SKILL.md"))];

pub(crate) fn builtin_skill_names() -> Vec<String> {
    let mut names: Vec<_> = BUILTIN.iter().map(|&(name, _)| name.to_owned()).collect();
    names.sort_unstable();
    names
}

pub(super) fn builtin_skill(name: &str) -> Option<&'static str> {
    BUILTIN
        .iter()
        .find_map(|&(candidate, content)| (candidate == name).then_some(content))
}
