import { useEffect, useState } from "preact/hooks";
import { call, on } from "../api";
import { openMenu, showError } from "../components/Dialog";
import { Hints } from "../components/Hints";
import { DiscIcon, ImageIcon } from "../components/Icons";
import { nav, useActions } from "../nav";
import {
  DrivePlan, formatBytes, formatEta, ImportItem, ImportProgress, isImportable, selectedBytes, statusLabel,
} from "./importFormat";

// Escuta desde antes de iniciar: um jogo pequeno pode terminar antes de a tela de progresso abrir.
let latest: ImportProgress | null = null;
const subscribers = new Set<(progress: ImportProgress) => void>();
let listening = false;

async function ensureListening(): Promise<void> {
  if (listening) return;
  listening = true;
  await on<ImportProgress>("import-progress", (progress) => {
    latest = progress;
    subscribers.forEach((notify) => notify(progress));
  });
}

/** Pergunta qual `.exe` usar nos jogos com mais de uma opção. Retorna null se a pessoa desistir. */
function askExe(item: ImportItem): Promise<string | null> {
  return new Promise((resolve) => {
    openMenu(
      `Qual executável abre ${item.title}?`,
      item.exe_options.map((exe) => ({ label: exe, onClick: () => resolve(exe) })),
      undefined,
      () => {
        nav.pop();
        resolve(null);
      },
    );
  });
}

async function runImport(plan: DrivePlan, ids: string[]): Promise<void> {
  const choices: Record<string, string> = {};
  for (const item of plan.items.filter((i) => ids.includes(i.id) && i.status === "needs_exe")) {
    const exe = await askExe(item);
    if (exe === null) return;
    choices[item.id] = exe;
  }
  try {
    await ensureListening();
    latest = null;
    await call("import_start", { drive: plan.drive, ids, choices });
    nav.push(<ImportRunning drive={plan.drive} />);
  } catch (e) {
    showError(e);
  }
}

function ImportRunning({ drive }: { drive: string }) {
  const [progress, setProgress] = useState<ImportProgress | null>(latest);
  const [cancelling, setCancelling] = useState(false);

  useEffect(() => {
    subscribers.add(setProgress);
    setProgress(latest);
    return () => void subscribers.delete(setProgress);
  }, []);

  const finished = progress?.finished ?? false;
  useActions(
    finished
      ? { confirm: () => nav.reset(), back: () => nav.reset() }
      : {
          confirm: () => {},
          back: () => {
            setCancelling(true);
            void call("import_cancel").catch(showError);
          },
        },
  );

  const percent = progress && progress.total_bytes > 0 ? Math.min(100, (progress.done_bytes / progress.total_bytes) * 100) : 0;

  if (progress && finished) {
    return (
      <>
        <div class="top-bar"><h1>{progress.cancelled ? "Importação cancelada" : "Importação concluída"}</h1></div>
        <div class="scroll">
          {progress.imported.length > 0 && (
            <>
              <h2>{progress.imported.length} item(ns) importado(s)</h2>
              <p>{progress.imported.join(", ")}</p>
              <p class="muted">Jogos e capas já estão na biblioteca. ISOs e assets estão em Menu → Arquivos de jogos.</p>
            </>
          )}
          {progress.cancelled && (
            <p class="muted">O que já terminou foi mantido. Conecte o disco de novo para continuar de onde parou.</p>
          )}
          {progress.errors.length > 0 && (
            <>
              <h2>Não foi possível importar</h2>
              {progress.errors.map((error) => <p class="error" key={error}>{error}</p>)}
              <p class="muted">O disco pode ter sido removido. Conecte-o de novo para continuar de onde parou.</p>
            </>
          )}
        </div>
        <Hints items={[["confirm", "Concluir"]]} />
      </>
    );
  }

  return (
    <>
      <div class="top-bar"><h1>Importando de {drive}</h1></div>
      <div class="session-screen">
        <h2>{progress ? `${progress.current} (${progress.item_index} de ${progress.item_count})` : "Preparando…"}</h2>
        <div class="progress-track"><div class="progress-fill" style={{ width: `${percent}%` }} /></div>
        {progress && (
          <p class="muted">
            {formatBytes(progress.done_bytes)} de {formatBytes(progress.total_bytes)} · {formatBytes(progress.bytes_per_sec)}/s · falta {formatEta(progress.total_bytes - progress.done_bytes, progress.bytes_per_sec)}
          </p>
        )}
        {cancelling && <p class="muted">Cancelando…</p>}
      </div>
      <Hints items={[["back", "Cancelar"]]} />
    </>
  );
}

function ImportList({ plan }: { plan: DrivePlan }) {
  const [selected, setSelected] = useState(() => new Set(plan.items.filter(isImportable).map((i) => i.id)));

  const start = () => {
    if (selected.size === 0) return;
    const needed = selectedBytes(plan.items, selected);
    if (plan.free_bytes !== null && needed > plan.free_bytes) {
      showError(`Espaço insuficiente: precisa de ${formatBytes(needed)} e há ${formatBytes(plan.free_bytes)} livres.`);
      return;
    }
    void runImport(plan, Array.from(selected));
  };

  useActions({ menu: start });

  const press = (item: ImportItem) => {
    if (!isImportable(item)) {
      const message = item.status === "done" ? "Este jogo já está na biblioteca." : item.problem ?? "";
      openMenu(item.title, [{ label: "OK", onClick: () => {} }], message);
      return;
    }
    const next = new Set(selected);
    if (next.has(item.id)) next.delete(item.id);
    else next.add(item.id);
    setSelected(next);
  };

  return (
    <>
      <div class="top-bar"><h1>Importar de {plan.drive}</h1></div>
      <p class="muted">
        Destino: {plan.library_dir}
        {plan.free_bytes !== null && ` · ${formatBytes(plan.free_bytes)} livres`}
        {` · marcado: ${formatBytes(selectedBytes(plan.items, selected))}`}
      </p>
      <div class="scroll list">
        {plan.items.length === 0 && <p class="muted">A pasta sofa\dump está vazia.</p>}
        {plan.items.map((item) => (
          <button key={item.id} onClick={() => press(item)}>
            <span>
              {isImportable(item) ? (selected.has(item.id) ? "☑ " : "☐ ") : ""}
              {item.category === "iso" && <DiscIcon />}
              {item.category === "capas" && <ImageIcon />}
              {item.title}
            </span>
            <span class={item.status === "blocked" || item.status === "conflict" ? "detail error" : "detail"}>
              {statusLabel(item)}
            </span>
          </button>
        ))}
      </div>
      <Hints items={[["confirm", "Marcar / detalhes"], ["menu", "Importar marcados"], ["back", "Voltar"]]} />
    </>
  );
}

function openPlan(plan: DrivePlan): void {
  nav.push(<ImportList plan={plan} />);
}

/** Menu → Importar jogos. */
export async function openImport(): Promise<void> {
  try {
    const plans = await call<DrivePlan[]>("import_scan");
    if (plans.length === 0) {
      openMenu(
        "Nenhum disco encontrado",
        [{ label: "OK", onClick: () => {} }],
        "Conecte um SSD ou pendrive com a pasta sofa\\dump na raiz. Veja o formato no README.",
      );
    } else if (plans.length === 1) {
      openPlan(plans[0]);
    } else {
      openMenu("Qual disco?", plans.map((plan) => ({
        label: `${plan.drive} (${plan.items.filter(isImportable).length} novos)`,
        onClick: () => openPlan(plan),
      })));
    }
  } catch (e) {
    showError(e);
  }
}

/** Aviso que aparece ao conectar um disco com jogos novos. */
export async function promptImport(found: { drive: string; count: number }): Promise<void> {
  const plans = await call<DrivePlan[]>("import_scan");
  const plan = plans.find((p) => p.drive === found.drive);
  if (!plan) return;
  const importable = plan.items.filter(isImportable);
  const total = importable.reduce((sum, i) => sum + i.pending_bytes, 0);
  openMenu(
    `Encontrei ${found.count} item(ns) novo(s) em ${found.drive}`,
    [
      {
        label: "Importar tudo",
        onClick: () => {
          if (plan.free_bytes !== null && total > plan.free_bytes) {
            showError(`Espaço insuficiente: precisa de ${formatBytes(total)} e há ${formatBytes(plan.free_bytes)} livres.`);
            return;
          }
          void runImport(plan, importable.map((i) => i.id));
        },
      },
      { label: "Escolher", onClick: () => openPlan(plan) },
      { label: "Agora não", onClick: () => {} },
    ],
    `${formatBytes(total)} serão copiados para ${plan.library_dir}. O disco não será alterado.`,
  );
}
