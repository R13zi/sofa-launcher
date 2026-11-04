import { nav, useActions } from "../nav";
import { Hints } from "./Hints";

export interface MenuItem {
  label: string;
  danger?: boolean;
  /** Mantém o menu aberto depois do clique. */
  keepOpen?: boolean;
  onClick: () => void;
}

function MenuDialog({ title, message, items, onBack }: { title: string; message?: string; items: MenuItem[]; onBack?: () => void }) {
  useActions(onBack ? { back: onBack } : {});
  return (
    <div class="dialog">
      <h1>{title}</h1>
      {message && <p class="muted">{message}</p>}
      <div class="actions">
        {items.map((item, i) => (
          <button
            key={item.label}
            class={item.danger ? "danger" : ""}
            data-autofocus={i === 0 ? "" : undefined}
            onClick={() => {
              if (!item.keepOpen) nav.pop();
              item.onClick();
            }}
          >
            {item.label}
          </button>
        ))}
      </div>
      <Hints items={[["confirm", "Selecionar"], ["back", "Voltar"]]} />
    </div>
  );
}

export function openMenu(title: string, items: MenuItem[], message?: string, onBack?: () => void): void {
  nav.push(<MenuDialog title={title} items={items} message={message} onBack={onBack} />, true);
}

export function showError(error: unknown): void {
  openMenu("Algo deu errado", [{ label: "OK", onClick: () => {} }], String(error));
}

export function confirmAction(title: string, confirmLabel: string, onConfirm: () => void, message?: string): void {
  // "Cancelar" vem primeiro para um toque acidental em A não executar a ação.
  openMenu(title, [
    { label: "Cancelar", onClick: () => {} },
    { label: confirmLabel, danger: true, onClick: onConfirm },
  ], message);
}
