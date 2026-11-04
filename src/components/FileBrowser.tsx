import { useEffect, useState } from "preact/hooks";
import { call, FsEntry } from "../api";
import { Action, afterRender, focusFirst, nav, useActions } from "../nav";
import { Hints } from "./Hints";

interface Props {
  title: string;
  /** Extensões aceitas, sem ponto. Vazio aceita qualquer arquivo. */
  exts?: string[];
  /** Escolhe uma pasta em vez de um arquivo. */
  pickDir?: boolean;
  onPick: (path: string) => void;
}

/** Explorador navegável pelo controle, já que o seletor do Windows não é. */
function FileBrowser({ title, exts = [], pickDir, onPick }: Props) {
  // Pilha de pastas abertas; vazia mostra os locais iniciais.
  const [trail, setTrail] = useState<string[]>([]);
  const [entries, setEntries] = useState<FsEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const current = trail[trail.length - 1];

  useEffect(() => {
    setError(null);
    const load = current
      ? call<FsEntry[]>("fs_list", { path: current, exts, dirsOnly: !!pickDir })
      : call<FsEntry[]>("fs_roots");
    load
      .then(setEntries)
      .catch((e) => {
        setEntries([]);
        setError(String(e));
      })
      .finally(() => afterRender(focusFirst));
  }, [current]);

  const pick = (path: string) => {
    nav.pop();
    onPick(path);
  };

  useActions({
    back: () => (trail.length ? setTrail(trail.slice(0, -1)) : nav.pop()),
    y: pickDir && current ? () => pick(current) : undefined,
  });

  return (
    <div class="dialog" style={{ width: "70vw", height: "85vh" }}>
      <h1>{title}</h1>
      <p class="muted">{current ?? "Escolha um local"}</p>
      {error && <p class="error">Não foi possível abrir esta pasta: {error}</p>}
      <div class="scroll list">
        {entries.length === 0 && !error && <p class="muted">Nada aqui.</p>}
        {entries.map((entry) => (
          <button
            key={entry.path}
            onClick={() => (entry.dir ? setTrail([...trail, entry.path]) : pick(entry.path))}
          >
            <span>{entry.dir ? "📁 " : "📄 "}{entry.name}</span>
          </button>
        ))}
      </div>
      <Hints
        items={[
          ["confirm", "Abrir"],
          ...(pickDir && current ? [["y", "Escolher esta pasta"] as [Action, string]] : []),
          ["back", "Voltar"],
        ]}
      />
    </div>
  );
}

export function openFileBrowser(props: Props): void {
  nav.push(<FileBrowser {...props} />, true);
}
