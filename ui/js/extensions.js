import { $, showToast } from "./util.js";
import { folderInput } from "./chat.js";
import { api, readSse } from "./api.js";

export async function loadExtensions() {
  const folder=folderInput.value.trim(); const cards=await api(`/api/extensions${folder?`?folder=${encodeURIComponent(folder)}`:""}`);
  const root=$("#extension-cards"); root.replaceChildren();
  for(const card of cards){const article=document.createElement("article");article.className="extension-card";
    const title=document.createElement("h2");title.textContent=card.name;const desc=document.createElement("p");desc.textContent=card.description;
    const link=document.createElement("a");link.href=card.homepage;link.target="_blank";link.rel="noopener";link.textContent="Trang chủ ↗";
    const badges=document.createElement("div");badges.className="extension-badges";const badge=(s)=>{const x=document.createElement("span");x.className="extension-badge";x.textContent=s;badges.append(x)};
    badge(card.installed?`Đã cài · ${card.version||"đã cài"}`:"Chưa cài");badge(`Codex: ${card.codex?"đã kết nối":"chưa"}`);badge(`Claude: ${card.claude?"đã kết nối":"chưa"}`);if(card.folder_indexed!==null)badge(`Thư mục hiện tại: ${card.folder_indexed?"đã lập chỉ mục":"chưa"}`);
    const actions=document.createElement("div");actions.className="extension-actions";
    for(const action of card.actions){if((action.id==="install"&&card.installed)||(action.id==="upgrade"&&!card.installed)||(action.id.startsWith("connect_")&&!card.installed)||(action.id==="index_folder"&&(!card.installed||!folder))||(action.id==="uninstall"&&!card.installed))continue;
      const button=document.createElement("button");button.className=action.id==="uninstall"?"danger-button":"subtle-button";button.textContent=action.label;button.addEventListener("click",()=>runExtensionAction(card,action));actions.append(button)}
    article.append(title,desc,link,badges,actions);root.append(article);
  }
}
export async function runExtensionAction(card,action){
  const command=`${action.command}${action.id==="index_folder"?`  (cwd: ${folderInput.value.trim()})`:""}`;
  if(!window.confirm(`Thực hiện: ${action.label}\n\nLệnh: ${command}\n\nCó thể sửa: ${action.modifies}\n\nTiếp tục?`))return;
  const log=$("#extension-log");log.textContent+=`\n> ${command}\n`;$("#extension-stop").hidden=false;
  try{const response=await fetch(`/api/extensions/${card.id}/actions/${action.id}`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({folder:folderInput.value.trim()})});if(!response.ok)throw new Error((await response.json()).error||`HTTP ${response.status}`);
    await readSse(response.body,(event,data)=>{if(event==="log")log.textContent+=`${data}\n`;if(event==="error"){showToast(`✗ ${data.message||"Lệnh thất bại"}`);log.textContent+=`${data.message||"Lệnh thất bại"}\n`}if(event==="done"&&data.ok)showToast("✓ Hoàn tất")});
  }catch(error){showToast(`✗ ${error.message}`);log.textContent+=`${error.message}\n`}finally{$("#extension-stop").hidden=true;await loadExtensions()}
}
