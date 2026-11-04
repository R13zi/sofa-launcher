import { useEffect, useState } from "preact/hooks";
import { call, on } from "../api";
import { openMenu, showError } from "../components/Dialog";
import { Hints } from "../components/Hints";
import { nav, useActions } from "../nav";
import { formatBytes } from "./importFormat";

interface UpdateInfo {
  current: string;
  version: string | null;
  notes: string | null;
}

interface UpdateProgress {
  done: number;
  total: number | null;
}

let latest: UpdateProgress | null = null;
const subscribers = new Set<(progress: UpdateProgress) => void>();
let listening = false;

async function ensureListening(): Promise<void> {
  if (listening) return;
  listening = true;
  await on<UpdateProgress>("update-progress", (progress) => {
    latest = progress;
    subscribers.forEach((notify) => notify(progress));
  });
}

function Installing() {
  const [progress, setProgress] = useState<UpdateProgress | null>(latest);
  useEffect(() => {
    subscribers.add(setProgress);
    setProgress(latest);
    return () => void subscribers.delete(setProgress);
  }, []);
  // Não dá para interromper com segurança: os botões não fazem nada aqui.
  useActions({ back: () => {}, confirm: () => {}, menu: () => {} });

  const percent = progress?.total ? Math.min(100, (progress.done / progress.total) * 100) : 0;
  const finished = progress?.total ? progress.done >= progress.total : false;
  return (
    <>
      <div class="top-bar"><h1>Atualizando o Sofa</h1></div>
      <div class="session-screen">
        <h2>{finished ? "Instalando… o Sofa vai reabrir sozinho." : "Baixando a atualização…"}</h2>
        <div class="progress-track"><div class="progress-fill" style={{ width: `${percent}%` }} /></div>
        {progress && (
          <p class="muted">
            {formatBytes(progress.done)}{progress.total ? ` de ${formatBytes(progress.total)}` : ""}
          </p>
        )}
      </div>
      <Hints items={[]} />
    </>
  );
}

async function install(): Promise<void> {
  await ensureListening();
  latest = null;
  nav.push(<Installing />);
  try {
    await call("update_install");
  } catch (e) {
    nav.pop();
    showError(e);
  }
}

/** Menu → Procurar atualizações. */
export async function checkForUpdates(): Promise<void> {
  try {
    const info = await call<UpdateInfo>("update_check");
    if (!info.version) {
      openMenu("Sofa está atualizado", [{ label: "OK", onClick: () => {} }], `Versão ${info.current}`);
      return;
    }
    openMenu(
      `Versão ${info.version} disponível`,
      [
        { label: "Atualizar agora", onClick: () => void install() },
        { label: "Agora não", onClick: () => {} },
      ],
      `Você está na versão ${info.current}. O Sofa fecha e reabre sozinho.${info.notes ? `\n\n${info.notes}` : ""}`,
    );
  } catch (e) {
    showError(`Não foi possível procurar atualizações. Verifique a internet.\n\n${e}`);
  }
}
