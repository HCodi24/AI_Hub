import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { configApi } from "@/apis/config.api";
import { petsApi } from "@/apis/pets.api";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import { BUNDLED_PETS, PET_KINDS } from "@/lib/pet-choice";
import type { Pet } from "@/schema/pets.schema";
import { settingsText as t } from "@/text/settings";
import styles from "@/components/features/settings/settings.module.css";
/** Settings › Tiện ích: pets are never bundled; they are downloaded on request or imported from the user's own folder. */
export function PetsSection() {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ["pets"], queryFn: ({ signal }) => petsApi.list(signal) });
  const config = useQuery({ queryKey: ["config"], queryFn: ({ signal }) => configApi.get(signal) });
  const [pending, setPending] = useState<Pet | null>(null);
  const [busy, setBusy] = useState("");
  const chosen = config.data?.pets ?? {};
  async function run(key: string, action: () => Promise<void>, done: string) {
    setPending(null); setBusy(key);
    try { await action(); if (done) notify(done); await client.invalidateQueries({ queryKey: ["pets"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
    finally { setBusy(""); }
  }
  async function choose(kind: string, pet: string) {
    const next = { ...chosen };
    if (pet) next[kind] = pet; else delete next[kind];
    try { await settingsApi.saveConfig({ pets: next }); await client.invalidateQueries({ queryKey: ["config"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
  }
  async function importFolder() {
    const path = await configApi.pickFolder().catch(() => null);
    if (!path) return;
    await run("import", async () => { await petsApi.import(path); }, t.pets.imported(path.split(/[\\/]/).filter(Boolean).at(-1) || path));
  }
  const others = list.data?.filter(pet => !(pet.id in BUNDLED_PETS)) ?? [];
  const installed = others.filter(pet => pet.installed);
  const sourceLabel = (pet: Pet) => pet.custom ? (pet.source === "Codex" ? t.pets.fromCodexFolder : t.pets.fromMutis) : pet.source;
  return <section aria-label={t.pets.title}><h3>{t.pets.title}</h3><p>{t.pets.intro}</p>
    <div className={styles.group}><h4>{t.pets.use}</h4><p>{t.pets.useHint}</p>
      {PET_KINDS.map(kind => <label key={kind}>{t.pets.kinds[kind]}
        <select value={chosen[kind] ?? ""} onChange={event => void choose(kind, event.target.value)}>
          <option value="">{t.pets.default}</option>
          {Object.entries(BUNDLED_PETS).map(([id, pet]) => <option key={id} value={id}>{pet.name}</option>)}
          {installed.map(pet => <option key={pet.id} value={pet.id}>{pet.name}</option>)}
        </select></label>)}
    </div>
    <div className={styles.actions}><Button variant="secondary" disabled={Boolean(busy)} onClick={() => void importFolder()}>{busy === "import" ? t.pets.downloading : t.pets.importFolder}</Button></div>
    <p className={styles.note}>{t.pets.importHint}</p>
    {Object.entries(BUNDLED_PETS).map(([id, pet]) => <article key={id} className={styles.group}>
      <header className={styles.cardHead}><h3>{pet.name}</h3><span className={styles.badge} data-on="true">{t.pets.own}</span>
        <span className={styles.badge} data-on="true">{t.pets.installed}</span></header>
      <p>{t.pets.from}: {t.pets.inApp}</p>{pet.note && <p>{pet.note}</p>}</article>)}
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {others.map(pet => <article key={pet.id} className={styles.group}>
      <header className={styles.cardHead}><h3>{pet.name}</h3>
        {pet.provider && <span className={styles.badge} data-on="true">{t.pets.forProvider(pet.provider)}</span>}
        {pet.custom && <span className={styles.badge} data-on="true">{t.pets.own}</span>}
        <span className={styles.badge} data-on={pet.installed}>{pet.installed ? t.pets.installed : t.pets.notInstalled}</span></header>
      {pet.notice && <p>{pet.notice}</p>}
      <p>{t.pets.from}: {sourceLabel(pet)}</p>
      {pet.installed && !pet.removable && !pet.custom && <p>{t.pets.fromCodex}</p>}
      <div className={styles.actions}>
        {!pet.installed && <Button variant="secondary" disabled={Boolean(busy)} onClick={() => setPending(pet)}>{busy === pet.id ? t.pets.downloading : t.pets.download}</Button>}
        {pet.removable && <Button variant="danger" disabled={Boolean(busy)} onClick={() => void run(pet.id, () => petsApi.remove(pet.id), t.pets.removed(pet.name))}>{t.pets.remove}</Button>}</div>
      {pending?.id === pet.id && <div className={styles.confirm} role="group"><p>{t.pets.confirm(pet.name, pet.source)}</p>
        <Button variant="primary" onClick={() => void run(pet.id, () => petsApi.install(pet.id), t.pets.downloaded(pet.name))}>{t.pets.download}</Button>
        <Button onClick={() => setPending(null)}>{t.cancel}</Button></div>}
    </article>)}</section>;
}
