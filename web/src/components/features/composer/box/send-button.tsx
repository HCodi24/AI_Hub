import { ArrowUp, Square } from "lucide-react";
import { Button } from "@/components/button/button";
import { chatText } from "@/text/chat";
import styles from "./box.module.css";

type Props = {
  streaming: boolean;
  disabled: boolean;
  onStop: () => void;
};

export function SendButton({ streaming, disabled, onStop }: Props) {
  return streaming ? (
    <Button className={styles.sendButton} variant="danger" onClick={onStop} aria-label={chatText.stop} title={chatText.stop}>
      <Square size={18} aria-hidden="true" />
    </Button>
  ) : (
    <Button className={styles.sendButton} variant="primary" type="submit" disabled={disabled} aria-label={chatText.send} title={chatText.send}>
      <ArrowUp size={20} aria-hidden="true" />
    </Button>
  );
}
