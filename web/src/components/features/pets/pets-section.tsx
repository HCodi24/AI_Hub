import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { petsApi } from "@/apis/pets.api";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Switch } from "@/components/button/switch";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import { PET_KINDS } from "@/lib/pet-choice";
import { settingsText as t } from "@/text/settings";
import styles from "@/components/features/settings/settings.module.css";
const TABS = ["chat", "garden"] as const;
type Tab = (typeof TABS)[number];
/** Settings › Pet: "Pet chat" (pets beside replies) and "Vườn pet" (a later feature, placeholder for now). */
export function PetsSection() {
  const [tab, setTab] = useState<Tab>("chat");
  function onTabKey(event: React.KeyboardEvent<HTMLButtonElement>) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    event.preventDefault(); const next = TABS[(TABS.indexOf(tab) + step + TABS.length) % TABS.length];
    setTab(next); document.getElementById(`pets-tab-${next}`)?.focus();
  }
  return <div className={styles.form}>
    <div className={styles.subTabs} role="tablist" aria-label={t.pets.tabsLabel}>
      {TABS.map(id => <button key={id} type="button" role="tab" id={`pets-tab-${id}`} aria-selected={tab === id} aria-controls={`pets-panel-${id}`}
        tabIndex={tab === id ? 0 : -1} onClick={() => setTab(id)} onKeyDown={onTabKey}>{t.pets.tabs[id]}</button>)}
    </div>
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
  const fileInput = useRef<HTMLInputElement>(null);
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
  async function importFolder() {
    const path = await configApi.pickFolder().catch(() => null);
    if (path) await run("import", async () => { await petsApi.import(path); }, t.pets.imported(path.split(/[\\/]/).filter(Boolean).at(-1) || path));
  }
  const pets = list.data ?? [];
  return <section className={styles.form} aria-label={t.pets.title}><p className={styles.note}>{t.pets.intro}</p>
    <label className={styles.check}><Switch checked={config.data?.pets_enabled ?? true} disabled={!config.data}
      onChange={event => void save({ pets_enabled: event.target.checked })} /><span>{t.pets.enable}</span></label>
    <div className={styles.group}><h4>{t.pets.use}</h4><p>{t.pets.useHint}</p>
      {PET_KINDS.map(kind => <label key={kind}>{t.pets.kinds[kind]}
        <select value={chosen[kind] ?? ""} onChange={event => choose(kind, event.target.value)}>
          <option value="">{t.pets.default}</option>
          {pets.map(pet => <option key={pet.id} value={pet.id}>{pet.name}</option>)}
        </select></label>)}
    </div>
    <div className={styles.actions}>
      <Button variant="secondary" disabled={Boolean(busy)} onClick={() => fileInput.current?.click()}>{busy === "upload" ? t.pets.adding : t.pets.uploadFile}</Button>
      <input ref={fileInput} type="file" accept="image/webp,.webp" hidden
        onChange={event => { const file = event.target.files?.[0]; if (file) void run("upload", () => petsApi.upload(file), t.pets.imported(file.name)); event.target.value = ""; }} />
      <Button variant="secondary" disabled={Boolean(busy)} onClick={() => void importFolder()}>{busy === "import" ? t.pets.adding : t.pets.importFolder}</Button></div>
    <p className={styles.note}>{t.pets.uploadHint}</p>
    <p className={styles.note}>{t.pets.importHint}</p>
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {list.data && !pets.length && <p>{t.pets.none}</p>}
    {pets.map(pet => <article key={pet.id} className={styles.group}>
      <header className={styles.cardHead}><h3>{pet.name}</h3></header>
      <div className={styles.actions}>
        <Button variant="danger" disabled={Boolean(busy)} onClick={() => void run(pet.id, () => petsApi.remove(pet.id), t.pets.removed(pet.name))}>{t.pets.remove}</Button></div>
    </article>)}</section>;
}
