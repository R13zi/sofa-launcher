import { useEffect, useState } from "preact/hooks";
import { currentFamily, Family, onFamilyChange } from "../input";
import { Action } from "../nav";

const GLYPHS: Record<Family, Partial<Record<Action, string>>> = {
  xbox: { confirm: "A", back: "B", x: "X", y: "Y", lb: "LB", rb: "RB", menu: "≡", view: "⧉" },
  nintendo: { confirm: "A", back: "B", x: "X", y: "Y", lb: "L", rb: "R", menu: "+", view: "−" },
  generic: { confirm: "A", back: "B", x: "X", y: "Y", lb: "LB", rb: "RB", menu: "Start", view: "Select" },
};

export function Hints({ items }: { items: [Action, string][] }) {
  const [family, setFamily] = useState(currentFamily());
  useEffect(() => onFamilyChange(setFamily), []);
  return (
    <div class="hints">
      {items.map(([action, label]) => (
        <span class="hint" key={action}>
          <span class="glyph">{GLYPHS[family][action]}</span>
          {label}
        </span>
      ))}
    </div>
  );
}
