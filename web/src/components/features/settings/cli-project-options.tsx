import { Switch } from "@/components/button/switch";
import styles from "./settings.module.css";

type ProjectOptions = { project_skills: boolean; project_instructions: boolean };

export function CliProjectOptions({ state, instructionsFile, disabled, onChange }: {
  state?: ProjectOptions;
  instructionsFile: string;
  disabled: boolean;
  onChange: (key: keyof ProjectOptions, checked: boolean) => void;
}) {
  return <div className={styles.cliSwitchTable}>
    <label className={`${styles.check} ${styles.cliSwitchRow}`}><span>Nạp skill của dự án</span>
      <Switch checked={state?.project_skills ?? false} disabled={!state || disabled}
        onChange={event => onChange("project_skills", event.target.checked)} /></label>
    <label className={`${styles.check} ${styles.cliSwitchRow}`}><span>Nạp hướng dẫn dự án ({instructionsFile})</span>
      <Switch checked={state?.project_instructions ?? false} disabled={!state || disabled}
        onChange={event => onChange("project_instructions", event.target.checked)} /></label>
  </div>;
}
