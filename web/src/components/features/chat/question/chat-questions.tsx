import { useId, useRef, useState, type FormEvent } from "react";
import { Button } from "@/components/button/button";
import { notify } from "@/components/toast/toast";
import { formatQuestionAnswers, parseChatQuestions, type ChatQuestion } from "@/lib/chat-questions";
import { useChatStore } from "../chat-store";
import { useCardStore } from "../card-store";
import type { SendReply } from "./quick-replies";
import styles from "./chat-questions.module.css";

export function ChatQuestions({ questions, send, active, chatId }: {
  questions: ChatQuestion[]; send: SendReply; active: boolean; chatId: string;
}) {
  const formId = useId();
  const [selected, setSelected] = useState<Record<string, number>>(() => Object.create(null));
  const [custom, setCustom] = useState<Record<string, string>>(() => Object.create(null));
  const [sending, setSending] = useState(false);
  const [stepIndex, setStep] = useState(0);
  const pending = useRef(false);
  const streaming = useChatStore(s => s.streaming);
  const running = useCardStore(s => s.running);
  const busy = sending || streaming || Boolean(running);
  const answers = Object.fromEntries(questions.map(q => [q.id,
    !q.options.length || selected[q.id] === -1 ? (typeof custom[q.id] === "string" ? custom[q.id].trim() : "") : q.options[selected[q.id]]?.label || "",
  ]));
  const complete = questions.every(q => Boolean(answers[q.id]));
  const multiple = questions.length > 1;
  const lastStep = Math.max(0, questions.length - 1);
  const step = Math.min(stepIndex, lastStep);
  const missing = multiple && !complete
    ? questions.flatMap((q, index) => answers[q.id] ? [] : [index + 1]).join(", ")
    : "";
  const status = sending ? "Đang gửi câu trả lời…" : complete ? "Sẵn sàng gửi"
    : multiple ? `Còn thiếu câu ${missing}` : "Trả lời từng câu hỏi rồi bấm gửi.";

  async function submit(event: FormEvent) {
    event.preventDefault();
    const state = useChatStore.getState();
    const latest = state.messages.at(-1);
    if (!active || !complete || pending.current || state.id !== chatId || state.streaming || useCardStore.getState().running
      || latest?.role !== "assistant" || JSON.stringify(parseChatQuestions(latest.content).questions) !== JSON.stringify(questions)) return;
    pending.current = true; setSending(true);
    try {
      const sent = await send(formatQuestionAnswers(questions, answers), true);
      if (!sent) notify("Chưa gửi được câu trả lời. Vui lòng thử lại.");
    } catch (error) { notify(error instanceof Error ? error.message : "Không gửi được câu trả lời."); }
    finally { pending.current = false; setSending(false); }
  }

  return <form className={styles.card} onSubmit={event => {
    if (step < lastStep) {
      event.preventDefault();
      setStep(step + 1);
    } else {
      void submit(event);
    }
  }} aria-label="Câu hỏi từ AI" aria-busy={sending}>
    <header className={styles.header}><strong>{active ? "Cần bạn lựa chọn" : "Câu hỏi từ AI"}</strong>
      <span>{active ? "AI chờ câu trả lời của bạn để tiếp tục." : "Xem câu trả lời trong các tin nhắn bên dưới."}</span>
      {multiple && <div className={styles.progress}>
        <span aria-live="polite" aria-atomic="true">Câu {step + 1}/{questions.length}</span>
        {questions.map((q, index) => <button key={q.id} type="button" className={styles.dot}
          aria-label={`Câu ${index + 1}`} aria-current={step === index ? "step" : undefined}
          data-answered={answers[q.id] ? true : undefined} onClick={() => setStep(index)} />)}
      </div>}
    </header>
    {questions.slice(step, step + 1).map(q => <fieldset key={q.id} className={styles.question} disabled={!active || busy}>
      <legend>{multiple ? `${step + 1}. ` : ""}{q.question}</legend>
      {q.options.map((option, index) => <label key={option.label} className={styles.option} data-selected={selected[q.id] === index}>
        <input type="radio" name={`${formId}-${q.id}`} checked={selected[q.id] === index}
          onChange={() => {
            setSelected(values => ({ ...values, [q.id]: index }));
            if (step < lastStep) setStep(step + 1);
          }} />
        <span><span className={styles.label}>{option.label}{option.recommended && <small className={styles.badge}>Đề xuất</small>}</span>
          {option.description && <span className={styles.description}>{option.description}</span>}</span>
      </label>)}
      {(active || (multiple && (selected[q.id] === -1 || (!q.options.length && Boolean(answers[q.id]))))) && <>
        {q.options.length > 0 && <label className={styles.option} data-selected={selected[q.id] === -1}>
          <input type="radio" name={`${formId}-${q.id}`} checked={selected[q.id] === -1}
            onChange={() => setSelected(values => ({ ...values, [q.id]: -1 }))} /><span>Ý kiến khác</span>
        </label>}
        {(!q.options.length || selected[q.id] === -1) && <label className={styles.custom}>
          <span>Câu trả lời của bạn</span>
          <textarea rows={3} maxLength={4000} value={typeof custom[q.id] === "string" ? custom[q.id] : ""}
            onChange={event => setCustom(values => ({ ...values, [q.id]: event.target.value }))} />
        </label>}
      </>}
    </fieldset>)}
    {(active || multiple) && <footer className={styles.footer}>
      {multiple && <div className={styles.navigation} role="group" aria-label="Điều hướng câu hỏi">
        <Button type="button" variant="ghost" disabled={step === 0} onClick={() => setStep(step - 1)}>Quay lại</Button>
        <Button type="button" disabled={step === lastStep} onClick={() => setStep(step + 1)}>Tiếp</Button>
      </div>}
      {active && <span role="status">{status}</span>}
      {active && step === lastStep && <Button type="submit" variant="primary" disabled={!complete || busy}>Gửi câu trả lời</Button>}
    </footer>}
  </form>;
}
