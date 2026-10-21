import { Help } from "./Help";
import { LayoutGrid, List, ChevronDown } from "lucide-react";
export type ExplorerMode =
  | "huge"
  | "large"
  | "medium"
  | "small"
  | "list"
  | "table"
  | "tiles"
  | "content"
  | "cards";
export const displayView = (value: ExplorerMode): "medium" | "table" =>
  ["list", "table"].includes(value) ? "table" : "medium";
export function ExplorerView(p: {
  value: ExplorerMode;
  change: (v: ExplorerMode) => void;
  lang: string;
}) {
  const ru = p.lang === "ru";
  const value = displayView(p.value);
  return (
    <div className="view-picker control-with-help">
      <div className="view-menu">
        {value === "table" ? <List size={15} /> : <LayoutGrid size={15} />}
        <select
          aria-label={ru ? "Вид отображения" : "Display view"}
          value={value}
          onChange={(e) => p.change(e.target.value as ExplorerMode)}
        >
          <option value="medium">{ru ? "Значки" : "Icons"}</option>
          <option value="table">{ru ? "Список" : "List"}</option>
        </select>
        <ChevronDown size={13} />
      </div>
      <Help topic="view" />
    </div>
  );
}
