import { useId, useRef, useState } from "react";
import { Blocks, Boxes, Hammer, LayoutDashboard, Palette, PawPrint, Puzzle, SlidersHorizontal, Users, Volume2 } from "lucide-react";
import { PetsSection } from "@/components/features/pets/pets-section";
import { Dialog } from "@/components/dialog/dialog";
import { ScrollArea } from "@/components/scroll-area";
import { Switch } from "@/components/button/switch";
import { MutisSlider } from "@/components/slider/mutis-slider";
import { useLiquidIndicator } from "@/components/features/settings/appearance/theme/glass/use-liquid-indicator";
import { chatSoundEnabled, chatSoundVolume, playChatSound, setChatSoundEnabled, setChatSoundVolume } from "@/lib/chat-sound";
import { settingsText as t } from "@/text/settings";
import { AddonsTab } from "./addons";
import { AppearanceTab } from "./appearance";
import { UpdateSection } from "./update-section";
import { DesktopBuild } from "./desktop-build";
import { ExtensionsTab } from "./extensions-tab";
import { GeneralTab } from "./general-tab";
import { MutisTab } from "./mutis";
import { ProvidersTab } from "./providers";
import styles from "./settings.module.css";
const TABS = [
  { id: "providers", icon: Boxes }, { id: "general", icon: SlidersHorizontal },
  { id: "appearance", icon: Palette },
  { id: "sound", icon: Volume2 }, { id: "mutis", icon: Users },
  { id: "addons", icon: Puzzle }, { id: "extensions", icon: Blocks }, { id: "pets", icon: PawPrint }, { id: "dev", icon: Hammer },
  { id: "dashboard", icon: LayoutDashboard },
] as const;
export type TabId = typeof TABS[number]["id"];
export const isTabId = (value: string | null): value is TabId => TABS.some(tab => tab.id === value);
type SettingsDialogProps = {
  /** Tab shown first; the preview page uses it to open the tab being edited. */
  initialTab?: TabId;
  folder: string;
  close: () => void;
  dashboardEnabled: boolean;
  onDashboardEnabledChange: (enabled: boolean) => void;
  taskBoardEnabled: boolean;
  onTaskBoardEnabledChange: (enabled: boolean) => void;
};
export function SettingsDialog({ initialTab = "providers", folder, close, dashboardEnabled, onDashboardEnabledChange, taskBoardEnabled, onTaskBoardEnabledChange }: SettingsDialogProps) {
  const [tab, setTab] = useState<TabId>(initialTab);
  const [soundEnabled, setSoundEnabled] = useState(chatSoundEnabled);
  const [volume, setVolume] = useState(chatSoundVolume);
  const volumeId = useId();
  const preview = () => { void playChatSound("done"); };
  const tabsRef = useRef<HTMLElement>(null);
  const tabIndicatorRef = useRef<HTMLSpanElement>(null);
  useLiquidIndicator(tabsRef, tabIndicatorRef, {
    selected: 'button[aria-current="page"]',
    item: "button",
    draggable: false,
  });
  return <Dialog title={t.title} close={close} wide className={styles.settingsDialog} iconClose>
    <div className={styles.layout}>
    <ScrollArea ref={tabsRef} as="nav" data-glass="settings-tabs" className={styles.tabs} aria-label={t.title}>
      <span ref={tabIndicatorRef} data-glass="settings-tab-indicator" aria-hidden="true" />
      {TABS.map(({ id, icon: Icon }) => <button key={id} id={`settings-tab-${id}`} aria-current={tab === id ? "page" : undefined}
        aria-controls="settings-panel" onClick={() => setTab(id)}><Icon size={20} aria-hidden="true" />{t.tabs[id]}</button>)}
    </ScrollArea>
    <ScrollArea as="section" id="settings-panel" aria-labelledby={`settings-tab-${tab}`} className={styles.panel} key={tab}>
      {tab !== "addons" && tab !== "providers" && tab !== "pets" && <h3 className={styles.panelTitle}>{t.tabs[tab]}</h3>}
      {tab === "appearance" && <AppearanceTab />}
      {tab === "providers" && <ProvidersTab />}{tab === "general" && <GeneralTab taskBoardEnabled={taskBoardEnabled} onTaskBoardEnabledChange={onTaskBoardEnabledChange} />}{tab === "mutis" && <MutisTab />}
      {tab === "addons" && <AddonsTab />}
      {tab === "extensions" && <ExtensionsTab folder={folder} />}{tab === "pets" && <PetsSection />}{tab === "dev" && <><UpdateSection /><DesktopBuild /></>}
      {tab === "sound" && <form className={styles.form}>
        <fieldset className={styles.group}><legend>{t.sound.title}</legend>
          <label className={styles.check}><Switch checked={soundEnabled}
            onChange={event => {
              const enabled = event.target.checked;
              setChatSoundEnabled(enabled);
              setSoundEnabled(enabled);
            }} /><span>{t.sound.enabled}</span></label>
          <label htmlFor={volumeId}>{t.sound.volume}</label>
          <div>
            <MutisSlider variant="smooth" formatValue={v => `${v}%`}
              id={volumeId} min={0} max={100} step={5} value={volume} disabled={!soundEnabled}
              aria-label={t.sound.volume} aria-valuetext={`${volume}%`}
              onChange={v => {
                setChatSoundVolume(v);
                setVolume(v);
              }} onPointerUp={preview} onKeyUp={preview} />
          </div>
          <p className={styles.note}>{t.sound.hint}</p>
        </fieldset>
      </form>}
      {tab === "dashboard" && <form className={styles.form}>
        <label className={styles.check}><Switch checked={dashboardEnabled}
          onChange={event => onDashboardEnabledChange(event.target.checked)} /><span>{t.dashboard.enabled}</span></label>
      </form>}
    </ScrollArea>
    </div>
  </Dialog>;
}
