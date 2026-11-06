import { call, GameSize, Installable, InstallTargets, SteamLibrary } from "../api";
import { runBusy } from "../components/Busy";
import { confirmAction, openMenu, showError } from "../components/Dialog";
import { openKeyboard } from "../components/VirtualKeyboard";
import { openStorage } from "./Storage";
import { formatSize } from "./updateFormat";

function uninstallNote(source: string): string {
  if (source === "steam") return "A Steam fecha e reabre em segundo plano durante a remoção.";
  if (source === "epic") return "O launcher da Epic é fechado durante a remoção e reaberto depois.";
  return "Os arquivos do jogo são apagados da biblioteca do Sofa.";
}

/** Confirma e desinstala. `onDone` roda depois que o jogo saiu da biblioteca. */
export async function uninstallGame(id: string, onDone: () => void): Promise<void> {
  const info = await call<GameSize | null>("game_size", { id }).catch(() => null);
  if (!info) {
    showError("Não foi possível descobrir onde este jogo está instalado.");
    return;
  }
  if (!info.removable) {
    openMenu(info.title, [{ label: "OK", onClick: () => {} }],
      "Este jogo está fora da biblioteca do Sofa. O Sofa só apaga jogos da Steam, da Epic e os importados.");
    return;
  }
  const freed = info.size > 0 ? `Libera ${formatSize(info.size)}. ` : "";
  confirmAction(`Desinstalar ${info.title}?`, "Desinstalar", async () => {
    try {
      await runBusy(`Desinstalando ${info.title}…`, () => call("game_uninstall", { id }), uninstallNote(info.source));
      window.dispatchEvent(new Event("sofa-library-changed"));
      onDone();
    } catch (e) {
      showError(e);
    }
  }, `${freed}${uninstallNote(info.source)}${info.source === "steam" || info.source === "epic" ? " Os saves na nuvem continuam na conta." : ""}`);
}

export function steamKeyPrompt(onSaved?: () => void): void {
  openKeyboard({
    title: "Chave da API Steam",
    secret: true,
    onDone: (key) => {
      if (!key.trim()) return;
      call("steam_key_save", { key }).then(() => onSaved?.()).catch(showError);
    },
  });
}

/** Disco de um caminho, como `D:`. */
const driveName = (path: string) => (/^[a-z]:/i.test(path) ? path.slice(0, 2).toUpperCase() : path);

/** Cabe no disco? Sem tamanho informado pela loja, não há como saber; deixa tentar. */
export const fitsIn = (size: number | null, free: number) => size === null || free >= size;

export function sizeText(size: number | null): string {
  return size === null ? "Tamanho não informado pela loja." : `Tamanho: cerca de ${formatSize(size)}.`;
}

function noSpace(game: Installable, size: number, target: SteamLibrary, onBack: () => void): void {
  openMenu("Falta espaço", [
    { label: "Abrir armazenamento", onClick: openStorage },
    { label: "Escolher outro disco", onClick: onBack },
    { label: "Cancelar", onClick: () => {} },
  ], `${game.title} precisa de cerca de ${formatSize(size)}, mas ${driveName(target.path)} tem só ${formatSize(target.free)} livres. `
    + `Desinstale jogos para liberar mais ${formatSize(size - target.free)} ou escolha outro disco.`);
}

async function installSteam(game: Installable, onDone: () => void): Promise<void> {
  const appid = game.id.replace(/^steam:/, "");
  const { size, targets } = await runBusy(`Verificando ${game.title}…`, () => call<InstallTargets>("steam_install_targets", { appid }),
    "Consultando o tamanho do jogo e o espaço dos discos.");
  if (targets.length === 0) {
    showError("Nenhum disco disponível para a Steam. A Steam está instalada?");
    return;
  }
  const start = (target: SteamLibrary) => confirmAction(
    `Instalar ${game.title}?`,
    "Instalar",
    async () => {
      try {
        await runBusy(`Preparando ${game.title}…`, () => call("steam_install", { appid, name: game.title, library: target.path, size }),
          "A Steam fecha e reabre em segundo plano. O download começa em seguida.");
        window.dispatchEvent(new Event("sofa-library-changed"));
        openMenu("Instalação iniciada", [{ label: "OK", onClick: () => {} }],
          `A Steam está baixando ${game.title}. O progresso aparece no cartão do jogo, na biblioteca.`);
        onDone();
      } catch (e) {
        showError(e);
      }
    },
    `${sizeText(size)} Em ${target.path} (${formatSize(target.free)} livres)`
      + `${target.new ? ", numa biblioteca nova da Steam" : ""}. A Steam fecha e reabre em segundo plano para começar o download.`,
  );
  const pick = () => {
    if (targets.length === 1) {
      choose(targets[0]);
      return;
    }
    openMenu("Instalar em qual disco?", targets.map((target) => ({
      label: `${driveName(target.path)} · ${formatSize(target.free)} livres${fitsIn(size, target.free) ? "" : " · não cabe"}`,
      onClick: () => choose(target),
    })), sizeText(size));
  };
  const choose = (target: SteamLibrary) => {
    if (size !== null && !fitsIn(size, target.free)) noSpace(game, size, target, pick);
    else start(target);
  };
  pick();
}

function installEpic(game: Installable): void {
  confirmAction(
    `Instalar ${game.title}?`,
    "Abrir a Epic",
    () => call("epic_install", { appName: game.id.replace(/^epic:/, "") }).catch(showError),
    "A janela de instalação da Epic vai abrir, com o tamanho do jogo e a pasta de destino (dá para trocar o disco ali). "
      + "Confirme com o cursor (analógico esquerdo e A); para voltar ao Sofa, aperte B.",
  );
}

export function installGame(game: Installable, onDone: () => void): void {
  if (game.source === "steam") void installSteam(game, onDone).catch(showError);
  else installEpic(game);
}
