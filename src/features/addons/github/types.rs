use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct LinkPlugin {
    pub name: String,
    pub description: String,
}

/// A marketplace found in a downloaded repository, for one CLI.
#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct LinkMarketplace {
    pub name: String,
    pub plugins: Vec<LinkPlugin>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LinkSkill {
    pub id: String,
    pub name: String,
    pub(in crate::features::addons) path: String,
}

impl LinkSkill {
    pub(in crate::features::addons) fn directory(&self) -> &Path {
        Path::new(&self.path).parent().expect("relative skill path")
    }
}

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct LinkFound {
    pub claude: Option<LinkMarketplace>,
    pub codex: Option<LinkMarketplace>,
    pub skills: Vec<LinkSkill>,
    pub mcp: bool,
}
