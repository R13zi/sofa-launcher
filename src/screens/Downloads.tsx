import { useEffect, useState } from "preact/hooks";
import { artUrl, call, Download, sourceLabel } from "../api";
import { MenuItem, openMenu, showError } from "../components/Dialog";
import { Hints } from "../components/Hints";
import { nav } from "../nav";
import {
  Completed, describeDownload, driveLabel, finished, fraction, Problem, problemOf, ProgressTracker, sameDay,
} from "./downloadsFormat";
import { GameDetail } from "./GameDetail";
import { refreshLibrary } from "./QuickMenu";
import { openStorage } from "./Storage";
import { uninstallGame } from "./storeActions";
import { formatSize } from "./updateFormat";

const POLL_MS = 3000;
const DONE_KEY = "sofa-downloads-done";

// Fica fora da tela para a velocidade e o "parado há X min" continuarem ao reabrir.
const tracker = new ProgressTracker();
let lastList: Download[] = [];

function loadCompleted(): Completed[] {
  try {
    const list = JSON.parse(localStorage.getItem(DONE_KEY) ?? "[]") as Completed[];
    return Array.isArray(list) ? list.filter((c) => sameDay(c.at, Date.now())) : [];
  } catch {
    return [];
  }
}

function saveCompleted(list: Completed[]): void {
  try {
    localStorage.setItem(DONE_KEY, JSON.stringify(list));
  } catch {
    // Sem armazenamento local, a lista de concluídos vale só enquanto o Sofa está aberto.
  }
}

/** Registra em "Concluídos hoje", mesmo com a tela fechada. */
export function addCompleted(id: string, title: string): void {
  const list = loadCompleted();
  if (list.some((c) => c.id === id)) return;
  saveCompleted([{ id, title, at: Date.now() }, ...list]);
  window.dispatchEvent(new Event("sofa-downloads-done"));
}

const clock = (ms: number) => new Date(ms).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });

function openOptions(d: Download, problem: Problem, onChanged: () => void): void {
  const items: MenuItem[] = [];
  if (problem?.kind === "no_space") items.push({ label: "Liberar espaço (Armazenamento)", onClick: openStorage });
  if (d.in_library) items.push({ label: "Ver detalhes do jogo", onClick: () => nav.push(<GameDetail gameId={d.id} />) });
  items.push({
    label: `Abrir na ${sourceLabel(d.source)} (modo desktop)`,
    onClick: () => call("store_open_downloads", { source: d.source }).catch(showError),
  });
  if (d.kind === "install" && d.in_library) {
    items.push({ label: "Cancelar instalação e apagar arquivos", danger: true, onClick: () => void uninstallGame(d.id, onChanged) });
  }
  const message = problem?.kind === "no_space"
    ? `Parado: ${d.drive ? driveLabel(d.drive) : "o disco"} encheu. Faltam ${formatSize(Math.max(0, d.total - d.done))} para terminar; há ${formatSize(d.free ?? 0)} livres.`
    : undefined;
  openMenu(d.title, items, message);
}

function DownloadRow({ d, problem, speed, autofocus, onChanged }: {
  d: Download; problem: Problem; speed: number | null; autofocus: boolean; onChanged: () => void;
}) {
  const part = fraction(d);
  const queued = d.status === "queued";
  const cover = artUrl(d.art);
  return (
    <button
      class={`download ${problem ? "attention" : ""}`}
      data-autofocus={autofocus ? "" : undefined}
      onClick={() => openOptions(d, problem, onChanged)}
    >
      <span class="download-cover">{cover && <img src={cover} alt="" />}</span>
      <span class="download-body">
        <span class="download-head">
          <span class="download-title">{d.title}</span>
          <span class="download-state">
            {problem?.kind === "no_space" ? "Parado · falta espaço"
              : problem?.kind === "stalled" ? "Parado"
              : queued ? "Na fila"
              : part !== null ? `${Math.floor(part * 100)}%` : ""}
          </span>
        </span>
        {!queued && (
          <span class={`download-bar ${part === null ? "indeterminate" : ""}`}>
            <span style={{ width: `${(part ?? 0.3) * 100}%` }} />
          </span>
        )}
        <span class="detail">{describeDownload(d, speed, problem)}</span>
      </span>
    </button>
  );
}

/** O que a Steam e a Epic estão baixando, com o progresso de cada jogo. */
export function Downloads() {
  const [list, setList] = useState<Download[] | null>(null);
  const [completed, setCompleted] = useState<Completed[]>(loadCompleted);

  const load = async () => {
    const current = await call<Download[]>("downloads").catch(() => null);
    if (!current) return;
    const now = Date.now();
    tracker.record(current, now);
    const done = finished(lastList, current, now);
    lastList = current;
    setList(current);
    if (done.length > 0) {
      setCompleted((prev) => {
        const next = [...done, ...prev.filter((c) => !done.some((d) => d.id === c.id))].filter((c) => sameDay(c.at, now));
        saveCompleted(next);
        return next;
      });
      void refreshLibrary().catch(() => {});
    }
  };

  useEffect(() => {
    void load();
    const timer = window.setInterval(() => void load(), POLL_MS);
    const reloadCompleted = () => setCompleted(loadCompleted());
    window.addEventListener("sofa-downloads-done", reloadCompleted);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("sofa-downloads-done", reloadCompleted);
    };
  }, []);

  const now = Date.now();
  const rows = (list ?? []).map((d) => ({ d, problem: problemOf(d, tracker.idleFor(d.id, now)), speed: tracker.speed(d.id) }));
  const attention = rows.filter((r) => r.problem);
  const active = rows.filter((r) => !r.problem && r.d.status === "active");
  const queued = rows.filter((r) => !r.problem && r.d.status === "queued");
  const ordered = [...active, ...attention, ...queued];
  const firstId = ordered[0]?.d.id;
  const reload = () => void load();

  const summary = [
    active.length && `${active.length} baixando`,
    queued.length && `${queued.length} na fila`,
    attention.length && `${attention.length} ${attention.length === 1 ? "parado" : "parados"}`,
  ].filter(Boolean).join(" · ");

  const section = (title: string, group: typeof rows) => group.length > 0 && (
    <section class="downloads-group">
      <h2>{title}</h2>
      {group.map((r) => (
        <DownloadRow key={r.d.id} d={r.d} problem={r.problem} speed={r.speed} autofocus={r.d.id === firstId} onChanged={reload} />
      ))}
    </section>
  );

  return (
    <>
      <div class="top-bar downloads-top">
        <div>
          <h1>Downloads</h1>
          {list !== null && <span class="muted">{summary || "Nada baixando agora"}</span>}
        </div>
      </div>
      <div class="scroll list downloads">
        {list === null && <p class="muted">Lendo os downloads da Steam e da Epic…</p>}
        {section("Baixando agora", active)}
        {section("Precisa de atenção", attention)}
        {section("Na fila", queued)}
        {completed.length > 0 && (
          <section class="downloads-group">
            <h2>Concluídos hoje</h2>
            {completed.map((c, i) => (
              <button
                key={c.id}
                data-autofocus={!firstId && i === 0 ? "" : undefined}
                onClick={() => nav.push(<GameDetail gameId={c.id} />)}
              >
                <span>{c.title}</span>
                <span class="detail done">Concluído às {clock(c.at)}</span>
              </button>
            ))}
          </section>
        )}
        {list !== null && list.length === 0 && completed.length === 0 && (
          <p class="muted">Quando a Steam ou a Epic estiverem baixando um jogo ou uma atualização, o progresso aparece aqui.</p>
        )}
      </div>
      <Hints items={[["confirm", "Opções"], ["back", "Voltar"]]} />
    </>
  );
}

export const openDownloads = () => nav.push(<Downloads />);
