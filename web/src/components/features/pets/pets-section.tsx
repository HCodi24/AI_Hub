import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, Trash2 } from "lucide-react";
import { configApi } from "@/apis/config.api";
import { petsApi } from "@/apis/pets.api";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Picker } from "@/components/picker/picker";
import { PillTabs } from "@/components/pill-tabs/pill-tabs";
import { Dropdown } from "@/components/select/select";
import { Switch } from "@/components/button/switch";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import { PET_KINDS } from "@/lib/pet-choice";
import { settingsText as t } from "@/text/settings";
import { PetThumb } from "./pet-avatar";
import styles from "@/components/features/settings/settings.module.css";
const TABS = ["chat", "garden"] as const;
type Tab = (typeof TABS)[number];
const PET_FAMILIES = ["codex", "claude", "deepseek"] as const;
type PetFamily = (typeof PET_FAMILIES)[number];
const PET_FAMILY_KINDS: Record<PetFamily, readonly (typeof PET_KINDS)[number][]> = {
  codex: ["codex-luna", "codex-sol", "codex-terra"],
  claude: ["claude-opus", "claude-sonnet", "claude-haiku", "claude-fable"],
  deepseek: ["deepseek"],
};
/** Settings › Pet: "Pet chat" (pets beside replies) and "Vườn pet" (a later feature, placeholder for now). */
export function PetsSection() {
  const [tab, setTab] = useState<Tab>("chat");
  const client = useQueryClient();
  const [busy, setBusy] = useState("");
  async function run(key: string, action: () => Promise<void>, done: string) {
    setBusy(key);
    try { await action(); notify(done); await client.invalidateQueries({ queryKey: ["pets"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
    finally { setBusy(""); }
  }
  return <div className={styles.form}>
    <header className={`${styles.addonHead} ${styles.providersHead}`}>
      <div className={styles.addonHeadText}><h3 className={styles.panelTitle}>{t.tabs.pets}</h3></div>
      <div className={styles.addonHeadActions}>
        <Picker mode="file" accept="image/webp,.webp" onPick={file => run("upload", () => petsApi.upload(file), t.pets.imported(file.name))}>
          {openPicker => <Button size="sm" variant="primary" disabled={Boolean(busy) || tab !== "chat"}
            aria-hidden={tab === "garden" || undefined} style={tab === "garden" ? { visibility: "hidden" } : undefined} onClick={openPicker}>
            <Plus size={16} aria-hidden="true" />{busy === "upload" ? t.pets.adding : t.pets.uploadFile}</Button>}
        </Picker>
      </div>
    </header>
    <PillTabs idPrefix="pets" label={t.pets.tabsLabel} value={tab} onChange={setTab}
      tabs={TABS.map(id => ({ id, label: t.pets.tabs[id] }))} />
    <div role="tabpanel" id={`pets-panel-${tab}`} aria-labelledby={`pets-tab-${tab}`} className={styles.form}>
      {tab === "chat" ? <ChatPets busy={busy} run={run} /> : <p className={styles.note}>{t.pets.gardenSoon}</p>}
    </div>
  </div>;
}
/** No pet ships with the app; the user uploads a WebP sheet or imports a pet folder, then picks one per source. */
function ChatPets({ busy, run }: { busy: string; run: (key: string, action: () => Promise<void>, done: string) => Promise<void> }) {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ["pets"], queryFn: ({ signal }) => petsApi.list(signal) });
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const [petFamily, setPetFamily] = useState<PetFamily>("codex");
  const chosen = config.data?.pets ?? {};
  async function save(body: { pets?: Record<string, string>; pets_enabled?: boolean }) {
    try { await settingsApi.saveConfig(body); await client.invalidateQueries({ queryKey: ["config"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
  }
  function choose(kind: string, pet: string) {
    // Rebuilt from the known families only: it drops keys left by older versions, which the server would reject.
    const next: Record<string, string> = {};
    for (const key of PET_KINDS) { const value = key === kind ? pet : chosen[key]; if (value) next[key] = value; }
    void save({ pets: next });
  }
  const pets = list.data ?? [];
  return <section className={styles.form} aria-label={t.pets.title}><p className={styles.note}>{t.pets.intro}</p>
    <label className={styles.check}><Switch checked={config.data?.pets_enabled ?? true} disabled={!config.data}
      onChange={event => void save({ pets_enabled: event.target.checked })} /><span>{t.pets.enable}</span></label>
    <div className={styles.group}><h4>{t.pets.use}</h4>
      <PillTabs idPrefix="pets-source" label={t.pets.familyTabsLabel} value={petFamily} onChange={setPetFamily}
        tabs={PET_FAMILIES.map(family => ({ id: family, label: t.pets.families[family] }))} />
      {PET_FAMILIES.map(family => <div key={family} role="tabpanel" id={`pets-source-panel-${family}`}
        aria-labelledby={`pets-source-tab-${family}`} hidden={petFamily !== family}>
        {PET_FAMILY_KINDS[family].filter(kind => PET_KINDS.includes(kind)).map(kind => <label key={kind}>{t.pets.kinds[kind]}
          <Dropdown value={chosen[kind] ?? ""} onChange={event => choose(kind, event.target.value)}>
            <option value="">{t.pets.default}</option>
            {pets.map(pet => <option key={pet.id} value={pet.id}>{pet.name}</option>)}
          </Dropdown></label>)}
      </div>)}
    </div>
    <p className={styles.note}>{t.pets.uploadHint}</p>
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {list.data && !pets.length && <p>{t.pets.none}</p>}
    {pets.length > 0 && (
      <ul className={styles.addonCard} aria-label={t.pets.title}>
        {pets.map(pet => <li key={pet.id} className={styles.addonRow}>
          <span className={styles.addonIcon} aria-hidden="true"><PetThumb id={pet.id} /></span>
          <span className={styles.addonText}><span className={styles.addonName}><strong>{pet.name}</strong></span></span>
          <span className={styles.addonControls}><Button size="sm" variant="ghost" disabled={Boolean(busy)} aria-label={`${t.pets.remove} ${pet.name}`} onClick={() => void run(pet.id, () => petsApi.remove(pet.id), t.pets.removed(pet.name))}>
            <Trash2 size={16} aria-hidden="true" />{t.pets.remove}</Button></span>
        </li>)}
      </ul>
    )}</section>;
}
