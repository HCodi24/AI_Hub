use serde::Serialize;
use std::{path::PathBuf, process::Command};

#[derive(Clone, Copy)]
pub struct Action { pub id: &'static str, pub label: &'static str, pub command: &'static str, pub args: &'static [&'static str], pub modifies: &'static str, pub needs_folder: bool, pub backup: Option<&'static str> }
pub struct Definition { pub id: &'static str, pub name: &'static str, pub description: &'static str, pub homepage: &'static str, pub executable: &'static str, pub version_args: &'static [&'static str], pub actions: &'static [Action] }
const ACTIONS: &[Action] = &[
 Action{id:"install",label:"Cài đặt",command:"npm",args:&["i","-g","@colbymchenry/codegraph"],modifies:"npm global packages",needs_folder:false,backup:None},
 Action{id:"connect_codex",label:"Kết nối Codex",command:"codegraph",args:&["install","--target=codex","--yes"],modifies:"~/.codex/config.toml",needs_folder:false,backup:Some("codex/config.toml")},
 Action{id:"connect_claude",label:"Kết nối Claude",command:"codegraph",args:&["install","--target=claude","--yes"],modifies:"~/.claude.json",needs_folder:false,backup:Some(".claude.json")},
 Action{id:"index_folder",label:"Lập chỉ mục thư mục",command:"codegraph",args:&["init"],modifies:"<thư mục>/.codegraph/",needs_folder:true,backup:None},
 Action{id:"upgrade",label:"Nâng cấp",command:"codegraph",args:&["upgrade"],modifies:"CodeGraph installation",needs_folder:false,backup:None},
 Action{id:"uninstall",label:"Gỡ cài đặt",command:"npm",args:&["rm","-g","@colbymchenry/codegraph"],modifies:"npm global packages",needs_folder:false,backup:None},
];
pub const CATALOG: &[Definition] = &[Definition{id:"codegraph",name:"CodeGraph",description:"Lập chỉ mục mã nguồn để hỗ trợ công cụ AI hiểu dự án.",homepage:"https://github.com/colbymchenry/codegraph",executable:"codegraph",version_args:&["--version"],actions:ACTIONS}];
#[derive(Serialize)] pub struct Card { pub id:&'static str,pub name:&'static str,pub description:&'static str,pub homepage:&'static str,pub installed:bool,pub version:Option<String>,pub codex:bool,pub claude:bool,pub folder_indexed:Option<bool>,pub actions:Vec<ActionView> }
#[derive(Serialize)] pub struct ActionView { pub id:&'static str,pub label:&'static str,pub command:String,pub modifies:&'static str }
fn find(name:&str)->Option<PathBuf>{
 let names=if cfg!(windows){vec![format!("{name}.exe"),format!("{name}.cmd"),name.to_string()]}else{vec![name.to_string()]};
 let mut dirs:Vec<PathBuf>=std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
 #[cfg(windows)] {for key in ["APPDATA","USERPROFILE","LOCALAPPDATA"] {if let Some(p)=std::env::var_os(key).map(PathBuf::from){dirs.push(match key{"APPDATA"=>p.join("npm"),"USERPROFILE"=>p.join(".codegraph").join("bin"),_=>p.join("codegraph").join("bin")});}}}
 names.iter().flat_map(|n|dirs.iter().map(move|d|d.join(n))).find(|p|p.is_file())
}
pub fn status(folder:Option<&str>)->Vec<Card>{CATALOG.iter().map(|d|{
 let exe=find(d.executable);let version=exe.as_ref().and_then(|p|Command::new(p).args(d.version_args).output().ok()).filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s|!s.is_empty());
 let home=dirs::home_dir().unwrap_or_default(); let codex=std::fs::read_to_string(home.join(".codex/config.toml")).map(|s|s.contains("[mcp_servers.codegraph")).unwrap_or(false);let claude=std::fs::read_to_string(home.join(".claude.json")).map(|s|s.contains("\"mcpServers\"")&&s.contains("\"codegraph\"")).unwrap_or(false);
 Card{id:d.id,name:d.name,description:d.description,homepage:d.homepage,installed:version.is_some(),version,codex,claude,folder_indexed:folder.map(|f|PathBuf::from(f).join(".codegraph/codegraph.db").is_file()),actions:d.actions.iter().map(|a|ActionView{id:a.id,label:a.label,command:format!("{} {}",a.command,a.args.join(" ")),modifies:a.modifies}).collect()}
 }).collect()}
pub fn action(id:&str,aid:&str)->Option<(&'static Definition,&'static Action)>{let d=CATALOG.iter().find(|d|d.id==id)?;Some((d,d.actions.iter().find(|a|a.id==aid)?))}
pub fn resolve(name:&str)->Option<PathBuf>{find(name)}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_actions_are_fixed_and_resolvable() {
        let (definition, install)=action("codegraph","install").expect("catalog install action");
        assert_eq!(definition.executable,"codegraph");
        assert_eq!(install.args,["i","-g","@colbymchenry/codegraph"]);
        assert!(action("codegraph","arbitrary-command").is_none());
    }
}
