import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { petsApi } from "@/apis/pets.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import { notify } from "@/components/toast/toast";
import type { Pet } from "@/schema/pets.schema";
import { settingsText as t } from "@/text/settings";
import styles from "@/components/features/settings/settings.module.css";
/** Settings › Tiện ích: pets that are not bundled and are downloaded only when the user asks. */
export function PetsSection() {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ["pets"], queryFn: ({ signal }) => petsApi.list(signal) });
  const [pending, setPending] = useState<Pet | null>(null);
  const [busy, setBusy] = useState("");
  async function run(pet: Pet, action: () => Promise<void>, done: string) {
    setPending(null); setBusy(pet.id);
    try { await action(); notify(done); await client.invalidateQueries({ queryKey: ["pets"] }); }
    catch (error) { notify(error instanceof Error ? error.message : t.error); }
    finally { setBusy(""); }
  }
  return <section aria-label={t.pets.title}><h3>{t.pets.title}</h3><p>{t.pets.intro}</p>
    {list.isPending && <Skeleton card />}{list.error && <p role="alert">{list.error.message}</p>}
    {list.data?.length === 0 && <p>{t.pets.none}</p>}
    {list.data?.map(pet => <article key={pet.id} className={styles.group}>
      <header className={styles.cardHead}><h3>{pet.name}</h3>
        <span className={styles.badge} data-on="true">{t.pets.forProvider(pet.provider)}</span>
        <span className={styles.badge} data-on={pet.installed}>{pet.installed ? t.pets.installed : t.pets.notInstalled}</span></header>
      <p>{pet.notice}</p>
      <p>{t.pets.from}: {pet.source}</p>
      {pet.installed && !pet.removable && <p>{t.pets.fromCodex}</p>}
      <div className={styles.actions}>
        {!pet.installed && <Button variant="secondary" disabled={Boolean(busy)} onClick={() => setPending(pet)}>{busy === pet.id ? t.pets.downloading : t.pets.download}</Button>}
        {pet.removable && <Button variant="danger" disabled={Boolean(busy)} onClick={() => void run(pet, () => petsApi.remove(pet.id), t.pets.removed(pet.name))}>{t.pets.remove}</Button>}</div>
      {pending?.id === pet.id && <div className={styles.confirm} role="group"><p>{t.pets.confirm(pet.name, pet.source)}</p>
        <Button variant="primary" onClick={() => void run(pet, () => petsApi.install(pet.id), t.pets.downloaded(pet.name))}>{t.pets.download}</Button>
        <Button onClick={() => setPending(null)}>{t.cancel}</Button></div>}
    </article>)}</section>;
}
