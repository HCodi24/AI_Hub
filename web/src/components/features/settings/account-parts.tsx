import { Button } from "@/components/button/button";
import { settingsText as t } from "@/text/settings";
import styles from "./settings.module.css";

export type SourceAccount = { connected: boolean; email?: string | null; plan?: string | null; origin?: "cli" | "web" | null };

/** "plus" -> "Plus", "claude_max" -> "Claude Max": the plan name as the CLI reports it, made readable. */
export const planLabel = (plan: string) => plan.split(/[_\s-]+/).filter(Boolean)
  .map(word => word[0]!.toUpperCase() + word.slice(1)).join(" ");

const accountName =(account: SourceAccount) => account.email || t.providers.account.unknownEmail;

/** The account this source signs in with: email, plan and where the sign-in came from. */
export function AccountSummary({ account }: { account?: SourceAccount }) {
  if (!account?.connected) return <p className={styles.account}>{t.providers.account.none}</p>;
  return <p className={styles.account}><strong>{accountName(account)}</strong>
    {account.plan && <span>{planLabel(account.plan)}</span>}
    {account.origin && <span>{t.providers.account.origin[account.origin]}</span>}</p>;
}

/** Asks before a sign-in replaces the account this source already uses. */
export function ReplaceConfirm({ account, disabled, onConfirm, onCancel }: {
  account: SourceAccount; disabled?: boolean; onConfirm: () => void; onCancel: () => void;
}) {
  return <div className={styles.confirm} role="alertdialog" aria-label={t.providers.account.replaceConfirm}>
    <p>{t.providers.account.replace(accountName(account))}</p>
    <Button variant="danger" size="sm" disabled={disabled} onClick={onConfirm}>{t.providers.account.replaceConfirm}</Button>
    <Button size="sm" disabled={disabled} onClick={onCancel}>{t.cancel}</Button>
  </div>;
}
