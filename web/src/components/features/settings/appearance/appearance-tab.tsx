import { StyleControl } from "@/components/features/settings/appearance/theme/style-control";
import { ThemeControl } from "@/components/features/settings/appearance/theme/theme-control";
import { settingsText as t } from "@/text/settings";
import styles from "./appearance.module.css";

export function AppearanceTab() {
  return <>
    <div className={styles.row}><span>{t.appearance.style}</span><StyleControl /></div>
    <div className={styles.row}><span>{t.appearance.theme}</span><ThemeControl /></div>
  </>;
}
