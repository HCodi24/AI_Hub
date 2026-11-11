import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { settingsApi } from "@/apis/settings.api";
import { Button } from "@/components/button/button";
import { Skeleton } from "@/components/skeleton/skeleton";
import type { ConnectionTest } from "@/schema/settings.schema";
import { settingsText as t } from "@/text/settings";
import { ProviderForm } from "./provider-form";
import { useSettingsConfig } from "./use-settings";
import styles from "./settings.module.css";
export function ProvidersTab() {
  const { query, apply } = useSettingsConfig();
  const cli = useQuery({ queryKey: ["cli-status"], queryFn: ({ signal }) => settingsApi.cliStatus(signal) });
  const [selected, setSelected] = useState<string | null>(null); // provider id, "" = new, null = nothing yet
  const [statuses, setStatuses] = useState<Record<string, ConnectionTest>>({});
  const providers = query.data?.providers || [];
  const firstId = providers[0]?.id;
  useEffect(() => { if (selected === null && firstId) setSelected(firstId); }, [selected, firstId]);
  // Check every source once when the tab opens so the dots are meaningful.
  const ids = providers.map(p => p.id).join(",");
  useEffect(() => {
    let cancelled = false;
    for (const id of ids ? ids.split(",") : []) {
      void settingsApi.testProvider(id).then(result => { if (!cancelled) setStatuses(current => ({ ...current, [id]: result })); }).catch(() => undefined);
    }
    return () => { cancelled = true; };
  }, [ids]);
  const current = providers.find(p => p.id === selected) || null;
  return <div className={styles.split}>
    <nav className={styles.list} aria-label={t.providers.list}>
      {query.isPending && <Skeleton />}
      {providers.map(provider => { const status = statuses[provider.id];
        return <button key={provider.id} aria-current={provider.id === selected ? "true" : undefined} onClick={() => setSelected(provider.id)}>
          <span className={styles.dot} data-state={status ? (status.ok ? "ok" : "error") : "unknown"} aria-hidden="true" />
          <span><strong>{provider.name}</strong><small>{status?.message || t.providers.notTested}</small></span></button>; })}
      <Button onClick={() => setSelected("")}><Plus size={16} aria-hidden="true" />{t.providers.add}</Button>
      <section className={styles.cli} aria-label={t.providers.cli}><h4>{t.providers.cli}</h4>
        {(["codex_cli", "claude_cli"] as const).map(key => { const item = cli.data?.[key];
          return <p key={key}><strong>{t.providers.kinds[key]}</strong><br />
            {item ? (item.installed ? item.version || t.providers.cliNoVersion : t.providers.cliMissing) : t.loading}
            {item?.path && <small title={item.path}>{item.path}</small>}</p>; })}
      </section>
    </nav>
    <section className={styles.detail}>
      {query.error && <p role="alert">{query.error.message}</p>}
      {selected === null ? <p className={styles.note}>{t.providers.selectHint}</p> : <ProviderForm key={selected || "new"} provider={current}
        status={current ? statuses[current.id] : undefined}
        onSaved={(config, id) => { apply(config); setSelected(id); }}
        onDeleted={config => { apply(config); setSelected(config.providers[0]?.id ?? null); }}
        onTested={(id, result) => setStatuses(state => ({ ...state, [id]: result }))} />}
    </section>
  </div>;
}
