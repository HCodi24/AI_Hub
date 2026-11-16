import { useState } from "react";
import { Blocks, Boxes, Hammer, LayoutDashboard, PawPrint, Puzzle, SlidersHorizontal, Users, Volume2 } from "lucide-react";
import { PetsSection } from "@/components/features/pets/pets-section";
import { Dialog } from "@/components/dialog/dialog";
import { ScrollArea } from "@/components/scroll-area";
import { Switch } from "@/components/button/switch";
import { settingsText as t } from "@/text/settings";
import { AddonsTab } from "./addons-tab";
import { DevTab } from "./dev-tab";
import { ExtensionsTab } from "./extensions-tab";
import { GeneralTab } from "./general-tab";
import { MutisTab } from "./mutis-tab";
import { ProvidersTab } from "./providers-tab";
import styles from "./settings.module.css";
const TABS = [
  { id: "providers", icon: Boxes }, { id: "general", icon: SlidersHorizontal }, { id: "sound", icon: Volume2 }, { id: "mutis", icon: Users },
  { id: "addons", icon: Puzzle }, { id: "extensions", icon: Blocks }, { id: "pets", icon: PawPrint }, { id: "dev", icon: Hammer },
  { id: "dashboard", icon: LayoutDashboard },
] as const;
type TabId = typeof TABS[number]["id"];
type SettingsDialogProps = {
  folder: string;
  close: () => void;
  dashboardEnabled: boolean;
  onDashboardEnabledChange: (enabled: boolean) => void;
  taskBoardEnabled: boolean;
  onTaskBoardEnabledChange: (enabled: boolean) => void;
};
export function SettingsDialog({ folder, close, dashboardEnabled, onDashboardEnabledChange, taskBoardEnabled, onTaskBoardEnabledChange }: SettingsDialogProps) {
  const [tab, setTab] = useState<TabId>("providers");
  return <Dialog title={t.title} close={close} wide className={styles.settingsDialog} iconClose>
    <div className={styles.layout}>
    <ScrollArea as="nav" className={styles.tabs} aria-label={t.title}>
      {TABS.map(({ id, icon: Icon }) => <button key={id} id={`settings-tab-${id}`} aria-current={tab === id ? "page" : undefined}
        aria-controls="settings-panel" onClick={() => setTab(id)}><Icon size={20} aria-hidden="true" />{t.tabs[id]}</button>)}
    </ScrollArea>
    <ScrollArea as="section" id="settings-panel" aria-labelledby={`settings-tab-${tab}`} className={styles.panel} key={tab}>
      <h3 className={styles.panelTitle}>{t.tabs[tab]}</h3>
      {tab === "providers" && <ProvidersTab />}{tab === "general" && <GeneralTab />}{tab === "mutis" && <MutisTab taskBoardEnabled={taskBoardEnabled} onTaskBoardEnabledChange={onTaskBoardEnabledChange} />}
      {tab === "addons" && <AddonsTab />}
      {tab === "extensions" && <ExtensionsTab folder={folder} />}{tab === "pets" && <PetsSection />}{tab === "dev" && <DevTab />}
      {tab === "sound" && <p className={styles.note}>{t.sound.notice}</p>}
      {tab === "dashboard" && <form className={styles.form}>
        <label className={styles.check}><Switch checked={dashboardEnabled}
          onChange={event => onDashboardEnabledChange(event.target.checked)} /><span>{t.dashboard.enabled}</span></label>
      </form>}
    </ScrollArea>
    </div>
  </Dialog>;
}
