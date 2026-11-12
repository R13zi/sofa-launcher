import { call, Download, SessionInfo, VolumeState } from "../api";
import { BatteryIndicator } from "../components/Battery";
import { VolumeBar } from "../components/VolumeBar";
import { confirmAction, MenuItem, openMenu, showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { openKeyboard } from "../components/VirtualKeyboard";
import { nav } from "../nav";
import { Emulators } from "./Emulators";
import { openGameFilesScreen } from "./GameFiles";
import { openImport } from "./ImportScreen";
import { Settings } from "./Settings";
import { openDownloads } from "./Downloads";
import { openStorage } from "./Storage";
import { checkForUpdates } from "./Updates";

/** Recarrega a varredura e avisa as telas abertas. */
export async function refreshLibrary(): Promise<void> {
  await call("library_refresh");
  window.dispatchEvent(new Event("sofa-library-changed"));
}

export function addGame(): void {
  openFileBrowser({
    title: "Escolher o .exe do jogo",
    exts: ["exe"],
    onPick: (path) => {
      const stem = path.split(/[\\/]/).pop()?.replace(/\.exe$/i, "") ?? "";
      openKeyboard({
        title: "Nome do jogo",
        initial: stem,
        onDone: async (title) => {
          try {
            await call("manual_add", { title: title.trim() || stem, path });
            await refreshLibrary();
          } catch (e) {
            showError(e);
          }
        },
      });
    },
  });
}

function powerMenu(): void {
  const act = (action: string) => () => call("power", { action }).catch(showError);
  openMenu("Energia", [
    { label: "Suspender", onClick: () => confirmAction("Suspender o computador?", "Suspender", act("suspend")) },
    { label: "Reiniciar", onClick: () => confirmAction("Reiniciar o computador?", "Reiniciar", act("restart")) },
    { label: "Desligar", onClick: () => confirmAction("Desligar o computador?", "Desligar", act("shutdown")) },
    { label: "Sair do Sofa", onClick: () => confirmAction("Sair do Sofa?", "Sair", act("exit")) },
  ]);
}

/** Menu aberto por Start ou pela combinação; com jogo aberto, mostra as opções da sessão primeiro. */
export async function openQuickMenu(): Promise<void> {
  const [session, volume, downloads] = await Promise.all([
    call<SessionInfo | null>("session_info"),
    call<VolumeState>("volume_get").catch(() => null),
    call<Download[]>("downloads").catch(() => []),
  ]);
  const downloading = downloads.filter((d) => d.status === "active").length;
  const items: MenuItem[] = [];
  const resume = () => call("session_resume").catch(showError);

  if (session) {
    items.push(
      { label: "Continuar", onClick: resume },
      {
        label: "Fechar jogo",
        danger: true,
        onClick: () => confirmAction(
          `Fechar ${session.title}?`,
          "Fechar jogo",
          () => call("session_close").catch(showError),
          "O progresso não salvo será perdido. Se o jogo não responder, ele será encerrado à força.",
        ),
      },
    );
  } else {
    items.push(
      { label: "Importar jogos (SSD ou pendrive)", onClick: () => void openImport() },
      { label: "Adicionar jogo (.exe)", onClick: addGame },
      { label: "Emuladores", onClick: () => nav.push(<Emulators />) },
      { label: downloading > 0 ? `Downloads · ${downloading} baixando` : "Downloads", onClick: openDownloads },
      { label: "Armazenamento", onClick: openStorage },
      { label: "Atualizar biblioteca", onClick: () => refreshLibrary().catch(showError) },
      { label: "Procurar atualizações do Sofa", onClick: () => void checkForUpdates() },
    );
  }
  items.push(
    { label: "Arquivos de jogos (ISOs e assets)", onClick: openGameFilesScreen },
    { label: "Modo desktop (mouse e teclado)", onClick: () => call("desktop_enter").catch(showError) },
    { label: "Configurações", onClick: () => nav.push(<Settings />) },
    { label: "Energia", onClick: powerMenu },
  );

  openMenu(session ? session.title : "Menu", items, undefined, session ? () => {
    nav.pop();
    void resume();
  } : undefined, {
    side: true,
    eyebrow: session ? "Jogando agora" : undefined,
    top: (
      <>
        <BatteryIndicator withName />
        <VolumeBar initial={volume} />
      </>
    ),
  });
}
