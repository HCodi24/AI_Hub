import { useEffect, useId, useState } from "react";
import { BackgroundControl } from "@/components/features/settings/appearance/theme/background-control";
import { StyleControl } from "@/components/features/settings/appearance/theme/style-control";
import { ThemeControl } from "@/components/features/settings/appearance/theme/theme-control";
import { ThicknessControl } from "@/components/features/settings/appearance/theme/thickness-control";
import { TransparencyControl } from "@/components/features/settings/appearance/theme/transparency-control";
import { settingsText as t } from "@/text/settings";
import styles from "./appearance.module.css";

export function AppearanceTab() {
  const [isGlass, setIsGlass] = useState(false);
  const transparencyId = useId();
  const thicknessId = useId();

  useEffect(() => {
    const root = document.documentElement;
    const sync = () => setIsGlass(root.dataset.style === "glass");
    sync();
    const observer = new MutationObserver(sync);
    observer.observe(root, { attributes: true, attributeFilter: ["data-style"] });
    return () => observer.disconnect();
  }, []);

  return <>
    <div className={styles.row}><span>{t.appearance.style}</span><StyleControl /></div>
    <div className={styles.row}><span>{t.appearance.theme}</span><ThemeControl /></div>
    {isGlass && <div className={styles.transparencyRow}>
      <label htmlFor={transparencyId}>{t.appearance.transparency}</label>
      <TransparencyControl id={transparencyId} />
    </div>}
    {isGlass && <div className={styles.transparencyRow}>
      <label htmlFor={thicknessId}>{t.appearance.thickness}</label>
      <ThicknessControl id={thicknessId} />
    </div>}
    {isGlass && <div className={styles.backgroundRow}><span>{t.appearance.background}</span><BackgroundControl /></div>}
  </>;
}
