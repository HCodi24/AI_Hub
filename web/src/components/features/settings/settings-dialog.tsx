import { useState } from "react";
import { Blocks, Boxes, Hammer, SlidersHorizontal, Users } from "lucide-react";
import { Dialog } from "@/components/dialog/dialog";
import { settingsText as t } from "@/text/settings";
import { DevTab } from "./dev-tab";
import { ExtensionsTab } from "./extensions-tab";
import { GeneralTab } from "./general-tab";
import { MutisTab } from "./mutis-tab";
import { ProvidersTab } from "./providers-tab";
import styles from "./settings.module.css";
const TABS = [
  { id: "providers", icon: Boxes }, { id: "general", icon: SlidersHorizontal }, { id: "mutis", icon: Users },
  { id: "extensions", icon: Blocks }, { id: "dev", icon: Hammer },
] as const;
type TabId = typeof TABS[number]["id"];
export function SettingsDialog({ folder, close }: { folder: string; close: () => void }) {
  const [tab, setTab] = useState<TabId>("providers");
  return <Dialog title={t.title} close={close} wide>
    <div className={styles.tabs} role="tablist" aria-label={t.title}>
      {TABS.map(({ id, icon: Icon }) => <button key={id} role="tab" id={`settings-tab-${id}`} aria-selected={tab === id}
        aria-controls="settings-panel" onClick={() => setTab(id)}><Icon size={16} aria-hidden="true" />{t.tabs[id]}</button>)}
    </div>
    <div id="settings-panel" role="tabpanel" aria-labelledby={`settings-tab-${tab}`} className={styles.panel}>
      {tab === "providers" && <ProvidersTab />}{tab === "general" && <GeneralTab />}{tab === "mutis" && <MutisTab />}
      {tab === "extensions" && <ExtensionsTab folder={folder} />}{tab === "dev" && <DevTab />}
    </div>
  </Dialog>;
}
