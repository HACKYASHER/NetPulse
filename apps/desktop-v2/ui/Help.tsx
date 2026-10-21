import {
  Children,
  isValidElement,
  useId,
  useRef,
  useState,
  useSyncExternalStore,
  type LabelHTMLAttributes,
} from "react";
import { createPortal } from "react-dom";
import { CircleHelp } from "lucide-react";
import { helpText, type HelpTopic } from "./settingHelpText";
let language = "ru";
const listeners = new Set<() => void>();
export function setHelpLanguage(value: string) {
  if (language === value) return;
  language = value;
  listeners.forEach((fn) => fn());
}
const subscribe = (fn: () => void) => {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
};
export function Help({ topic }: { topic: HelpTopic }) {
  const lang = useSyncExternalStore(subscribe, () => language);
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  const id = useId();
  const text = helpText(topic, lang);
  const rect = anchor.current?.getBoundingClientRect();
  return (
    <span
      ref={anchor}
      className="setting-help"
      role="button"
      tabIndex={0}
      aria-label={lang === "ru" ? "Пояснение настройки" : "Setting explanation"}
      aria-describedby={open ? id : undefined}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setOpen((v) => !v);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") setOpen(false);
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          setOpen((v) => !v);
        }
      }}
    >
      <CircleHelp size={14} />
      {open &&
        rect &&
        createPortal(
          <span
            role="tooltip"
            id={id}
            className="setting-tooltip"
            style={{
              left: Math.max(
                8,
                Math.min(
                  rect.left,
                  window.innerWidth - Math.min(360, window.innerWidth - 16) - 8,
                ),
              ),
              ...(rect.bottom + 170 < window.innerHeight
                ? { top: rect.bottom + 8 }
                : { bottom: window.innerHeight - rect.top + 8 }),
            }}
          >
            {text}
          </span>,
          document.body,
        )}
    </span>
  );
}
export function HelpLabel({
  topic,
  children,
  ...props
}: LabelHTMLAttributes<HTMLLabelElement> & { topic: HelpTopic }) {
  const all = Children.toArray(children);
  const control = (v: (typeof all)[number]) =>
    isValidElement(v) &&
    ["input", "select", "textarea"].includes(String(v.type));
  const check = all.some(
    (v) =>
      isValidElement<{ type?: string }>(v) &&
      ["checkbox", "radio"].includes(v.props.type ?? ""),
  );
  return (
    <label {...props}>
      {check ? (
        <>
          {children}
          <Help topic={topic} />
        </>
      ) : (
        <>
          <span className="field-caption">
            {all.filter((v) => !control(v))}
            <Help topic={topic} />
          </span>
          {all.filter(control)}
        </>
      )}
    </label>
  );
}
