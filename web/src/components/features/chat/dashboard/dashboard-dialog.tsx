import { Dialog } from "@/components/dialog/dialog";
import { chatText } from "@/text/chat";
import { TokenDashboard } from "./token-dashboard";
import styles from "./dashboard-dialog.module.css";

/**
 * Large Dashboard dialog opened from the chat frame.
 * The body mounts the full token dashboard of the active chat: cumulative chart, totals, exact table,
 * explanatory notes and the backend history. Escape handling and focus restoration come from the
 * shared Dialog.
 */
export function DashboardDialog({ close }: { close: () => void }) {
  return <Dialog title={chatText.dashboard.title} close={close} wide iconClose className={styles.dashboardDialog}>
    <div data-glass="dashboard-body" className={styles.body}><TokenDashboard /></div>
  </Dialog>;
}
