import { useEffect, useRef } from "react";
import { chatText } from "@/text/chat";
import styles from "./composer.module.css";

type SkillSuggestionsProps = {
  id: string;
  names: readonly string[];
  activeIndex: number;
  onSelect: (name: string) => void;
};

export function SkillSuggestions({ id, names, activeIndex, onSelect }: SkillSuggestionsProps) {
  const activeOption = useRef<HTMLLIElement>(null);

  useEffect(() => {
    activeOption.current?.scrollIntoView({ block: "nearest" });
  }, [activeIndex, names]);

  if (names.length === 0) return null;

  return <div data-glass="skill-menu" className={styles.skillMenu}>
    <ul id={id} className={styles.skillSuggestions} role="listbox" aria-label={chatText.skillSuggestions}>
      {names.map((name, index) => {
        const optionId = `${id}-${index}`;
        const selected = index === activeIndex;
        return (
          <li
            key={optionId}
            id={optionId}
            ref={selected ? activeOption : undefined}
            role="option"
            aria-selected={selected}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => onSelect(name)}
          >
            /{name}
          </li>
        );
      })}
    </ul>
    <p id={`${id}-hint`} className={styles.skillHint}>
      <span><kbd>↑</kbd> <kbd>↓</kbd> {chatText.skillNavigate}</span>
      <span><kbd>Enter</kbd> {chatText.skillSelect}</span>
    </p>
  </div>;
}
