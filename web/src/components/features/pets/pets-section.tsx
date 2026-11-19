import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { petsApi } from "@/apis/pets.api";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Picker } from "@/components/picker/picker";
import { PillTabs } from "@/components/pill-tabs/pill-tabs";
import { Switch } from "@/components/button/switch";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import { PET_KINDS } from "@/lib/pet-choice";
import { settingsText as t } from "@/text/settings";
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
  return <div className={styles.form}>
    <PillTabs idPrefix="pets" label={t.pets.tabsLabel} value={tab} onChange={setTab}
      tabs={TABS.map(id => ({ id, label: t.pets.tabs[id] }))} />
    <div role="tabpanel" id={`pets-panel-${tab}`} aria-labelledby={`pets-tab-${tab}`} className={styles.form}>
      {tab === "chat" ? <ChatPets /> : <p className={styles.note}>{t.pets.gardenSoon}</p>}
    </div>
  </div>;
}
/** No pet ships with the app; the user uploads a WebP sheet or imports a pet folder, then picks one per source. */
function ChatPets() {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ["pets"], queryFn: ({ signal }) => petsApi.list(signal) });
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const [busy, setBusy] = useState("");
  const [petFamily, setPetFamily] = useState<PetFamily>("codex");
  const chosen = config.data?.pets ?? {};
  async function run(key: string, action: () => Promise<void>, done: string) {
    setBusy(key);
    try { await action(); notify(done); await client.invalidateQueries({ queryKey: ["pets"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
    finally { setBusy(""); }
  }
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
    <div className={styles.group}><h4>{t.pets.use}</h4><p>{t.pets.useHint}</p>
      <PillTabs idPrefix="pets-source" label={t.pets.familyTabsLabel} value={petFamily} onChange={setPetFamily}
        tabs={PET_FAMILIES.map(family => ({ id: family, label: t.pets.families[family] }))} />
      {PET_FAMILIES.map(family => <div key={family} role="tabpanel" id={`pets-source-panel-${family}`}
        aria-labelledby={`pets-source-tab-${family}`} hidden={petFamily !== family}>
        {PET_FAMILY_KINDS[family].filter(kind => PET_KINDS.includes(kind)).map(kind => <label key={kind}>{t.pets.kinds[kind]}
          <select value={chosen[kind] ?? ""} onChange={event => choose(kind, event.target.value)}>
            <option value="">{t.pets.default}</option>
            {pets.map(pet => <option key={pet.id} value={pet.id}>{pet.name}</option>)}
          </select></label>)}
      </div>)}
    </div>
    <div className={styles.actions}>
      <Picker mode="file" accept="image/webp,.webp" onPick={file => run("upload", () => petsApi.upload(file), t.pets.imported(file.name))}>
        {openPicker => <Button variant="secondary" disabled={Boolean(busy)} onClick={openPicker}>{busy === "upload" ? t.pets.adding : t.pets.uploadFile}</Button>}
      </Picker>
      <Picker mode="folder" onPick={path => run("import", async () => { await petsApi.import(path); }, t.pets.imported(path.split(/[\\/]/).filter(Boolean).at(-1) || path))}>
        {openPicker => <Button variant="secondary" disabled={Boolean(busy)} onClick={openPicker}>{busy === "import" ? t.pets.adding : t.pets.importFolder}</Button>}
      </Picker></div>
    <p className={styles.note}>{t.pets.uploadHint}</p>
    <p className={styles.note}>{t.pets.importHint}</p>
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {list.data && !pets.length && <p>{t.pets.none}</p>}
    {pets.length > 0 && (
      <ul className={styles.rows}>
        {pets.map(pet => <li key={pet.id}>
          <span className={styles.rowText}><strong>{pet.name}</strong></span>
          <Button variant="danger" size="sm" disabled={Boolean(busy)} onClick={() => void run(pet.id, () => petsApi.remove(pet.id), t.pets.removed(pet.name))}>{t.pets.remove}</Button>
        </li>)}
      </ul>
    )}</section>;
}
