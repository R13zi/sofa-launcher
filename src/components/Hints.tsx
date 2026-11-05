import { useEffect, useState } from "preact/hooks";
import { currentFamily, Family, onFamilyChange } from "../input";
import { Action, dispatchAction } from "../nav";

const GLYPHS: Record<Family, Partial<Record<Action, string>>> = {
  xbox: { confirm: "A", back: "B", x: "X", y: "Y", lb: "LB", rb: "RB", menu: "≡", view: "⧉" },
  nintendo: { confirm: "A", back: "B", x: "X", y: "Y", lb: "L", rb: "R", menu: "+", view: "−" },
  generic: { confirm: "A", back: "B", x: "X", y: "Y", lb: "LB", rb: "RB", menu: "Start", view: "Select" },
  keyboard: { confirm: "Enter", back: "Esc", x: "X", y: "F", lb: "Q", rb: "E", menu: "M", view: "V" },
};

/** Cada dica também é clicável, o que dá ao mouse as mesmas ações dos botões do controle. */
export function Hints({ items }: { items: [Action, string, string?][] }) {
  const [family, setFamily] = useState(currentFamily());
  useEffect(() => onFamilyChange(setFamily), []);
  return (
    <div class="hints">
      {items.map(([action, label, glyph]) => (
        <span class="hint" key={action} onClick={() => dispatchAction(action)}>
          <span class="glyph">{glyph ?? GLYPHS[family][action]}</span>
          {label}
        </span>
      ))}
    </div>
  );
}
