#[derive(Clone, Copy)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub modifies: &'static str,
    pub needs_folder: bool,
    pub backup: Option<&'static str>,
}

pub struct Definition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub homepage: &'static str,
    pub executable: &'static str,
    pub version_args: &'static [&'static str],
    pub actions: &'static [Action],
}

const ACTIONS: &[Action] = &[
    Action {
        id: "install",
        label: "Cài đặt",
        command: "npm",
        args: &["i", "-g", "@colbymchenry/codegraph"],
        modifies: "npm global packages",
        needs_folder: false,
        backup: None,
    },
    Action {
        id: "connect_codex",
        label: "Kết nối Codex",
        command: "codegraph",
        args: &["install", "--target=codex", "--yes"],
        modifies: "~/.codex/config.toml",
        needs_folder: false,
        backup: Some("codex/config.toml"),
    },
    Action {
        id: "connect_claude",
        label: "Kết nối Claude",
        command: "codegraph",
        args: &["install", "--target=claude", "--yes"],
        modifies: "~/.claude.json",
        needs_folder: false,
        backup: Some(".claude.json"),
    },
    Action {
        id: "index_folder",
        label: "Lập chỉ mục thư mục",
        command: "codegraph",
        args: &["init"],
        modifies: "<thư mục>/.codegraph/",
        needs_folder: true,
        backup: None,
    },
    Action {
        id: "upgrade",
        label: "Nâng cấp",
        command: "codegraph",
        args: &["upgrade"],
        modifies: "CodeGraph installation",
        needs_folder: false,
        backup: None,
    },
    Action {
        id: "uninstall",
        label: "Gỡ cài đặt",
        command: "npm",
        args: &["rm", "-g", "@colbymchenry/codegraph"],
        modifies: "npm global packages",
        needs_folder: false,
        backup: None,
    },
];

pub const CATALOG: &[Definition] = &[Definition {
    id: "codegraph",
    name: "CodeGraph",
    description: "Lập chỉ mục mã nguồn để hỗ trợ công cụ AI hiểu dự án.",
    homepage: "https://github.com/colbymchenry/codegraph",
    executable: "codegraph",
    version_args: &["--version"],
    actions: ACTIONS,
}];

pub fn action(id: &str, action_id: &str) -> Option<(&'static Definition, &'static Action)> {
    let definition = CATALOG.iter().find(|definition| definition.id == id)?;
    Some((
        definition,
        definition
            .actions
            .iter()
            .find(|action| action.id == action_id)?,
    ))
}
