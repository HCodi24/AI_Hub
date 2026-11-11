import { useQuery } from "@tanstack/react-query";
import { mutisApi } from "@/apis/mutis.api";
import type { MutisStats as Stats } from "@/schema/mutis.schema";
import { settingsText as t } from "@/text/settings";
import styles from "./settings.module.css";
const number = (value: number) => value.toLocaleString("vi-VN");
function Table({ title, groups }: { title: string; groups: Stats["providers"] }) {
  const rows = Object.entries(groups);
  if (!rows.length) return null;
  return <div className={styles.tableWrap}><table className={styles.table}><caption>{title}</caption>
    <thead><tr><th scope="col">{t.mutis.statSource}</th><th scope="col">{t.mutis.statTasks}</th><th scope="col">{t.mutis.statSuccess}</th>
      <th scope="col">{t.mutis.statAvg}</th><th scope="col">{t.mutis.statTokens}</th><th scope="col">{t.mutis.statRetries}</th></tr></thead>
    <tbody>{rows.map(([name, group]) => <tr key={name}><th scope="row">{name}</th><td>{number(group.tasks_run)}</td>
      <td>{Math.round(group.success_rate * 100)}%</td><td>{number(Math.round(group.avg_duration_ms / 1000))}</td>
      <td>{number(group.total_tokens)}</td><td>{number(group.retries)}</td></tr>)}</tbody></table></div>;
}
export function MutisStats() {
  const stats = useQuery({ queryKey: ["mutis-stats"], queryFn: ({ signal }) => mutisApi.stats(30, signal) });
  const empty = stats.data && !Object.keys(stats.data.providers).length && !Object.keys(stats.data.task_kinds).length;
  return <section aria-label={t.mutis.stats}><h3>{t.mutis.stats}</h3>
    {stats.isPending && <p className={styles.note}>{t.loading}</p>}
    {stats.error && <p role="alert">{stats.error.message}</p>}
    {empty && <p className={styles.note}>{t.mutis.noStats}</p>}
    {stats.data && <><Table title={t.mutis.byProvider} groups={stats.data.providers} /><Table title={t.mutis.byKind} groups={stats.data.task_kinds} /></>}
  </section>;
}
