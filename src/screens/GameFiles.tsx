import { useEffect, useState } from "preact/hooks";
import { call, GameView } from "../api";
import { MenuItem, openMenu, showError } from "../components/Dialog";
import { Hints } from "../components/Hints";
import { nav } from "../nav";
import { filesForGame, formatBytes, GameFilesEntry } from "./importFormat";

export const fileName = (path: string) => path.split("\\").pop() ?? path;

async function copyPath(path: string): Promise<void> {
  try {
    await call("clipboard_copy", { text: path });
    openMenu(
      "Caminho copiado",
      [{ label: "OK", onClick: () => {} }],
      `${path}\n\nPara colar no jogo: segure L3 + R3, abra o Modo desktop, aperte Y para o teclado e use Colar.`,
    );
  } catch (e) {
    showError(e);
  }
}

async function setLaunchFile(game: GameView, file: string, onChanged?: () => void): Promise<void> {
  try {
    await call("set_launch_file", { id: game.id, path: file });
    onChanged?.();
    openMenu(
      "Arquivo definido",
      [{ label: "OK", onClick: () => {} }],
      `${game.title} vai iniciar com:\n${file}\n\nPara trocar ou remover, abra os detalhes do jogo.`,
    );
  } catch (e) {
    showError(e);
  }
}

/** Só `.exe` adicionados recebem o arquivo; emuladores e lojas têm seu próprio jeito de iniciar. */
const acceptsFile = (game: GameView) => game.source === "manual";

/** Fora dos detalhes de um jogo: usa o de mesmo nome ou pergunta qual. */
async function chooseGameFor(entry: GameFilesEntry, file: string): Promise<void> {
  try {
    const games = (await call<GameView[]>("library_list")).filter(acceptsFile);
    if (games.length === 0) {
      showError("Nenhum jogo em .exe na biblioteca. Emuladores e lojas não usam este recurso.");
      return;
    }
    const same = games.find((game) => filesForGame(game.title, [entry]));
    if (same) {
      await setLaunchFile(same, file);
      return;
    }
    openMenu("Usar em qual jogo?", games.map((game) => ({
      label: game.title,
      onClick: () => void setLaunchFile(game, file),
    })));
  } catch (e) {
    showError(e);
  }
}

/** Opções de um item: usar ao iniciar e copiar o caminho, das imagens encontradas para a pasta. */
export function openGameFilesEntry(entry: GameFilesEntry, game?: GameView, onChanged?: () => void): void {
  const isFile = entry.main_files.length === 1 && entry.main_files[0] === entry.path;
  const paths = isFile ? [entry.path] : [...entry.main_files, entry.path];
  const label = (path: string) => (path === entry.path && !isFile ? "pasta inteira" : fileName(path));

  const items: MenuItem[] = [];
  if (!game || acceptsFile(game)) {
    for (const path of paths) {
      items.push({
        label: `Usar ao iniciar: ${label(path)}`,
        onClick: () => void (game ? setLaunchFile(game, path, onChanged) : chooseGameFor(entry, path)),
      });
    }
  }
  for (const path of paths) {
    items.push({ label: `Copiar caminho: ${label(path)}`, onClick: () => void copyPath(path) });
  }
  openMenu(entry.name, items, `${entry.file_count} arquivo(s), ${formatBytes(entry.size)}\n${entry.path}`);
}

export function GameFilesScreen() {
  const [entries, setEntries] = useState<GameFilesEntry[] | null>(null);
  useEffect(() => {
    call<GameFilesEntry[]>("game_files_list").then(setEntries).catch(showError);
  }, []);

  if (!entries) return <p class="muted">Carregando…</p>;

  return (
    <>
      <div class="top-bar"><h1>Arquivos de jogos</h1></div>
      <p class="muted">ISOs e pastas de assets que os jogos pedem depois, como nos decomps.</p>
      <div class="scroll list">
        {entries.length === 0 && (
          <p class="muted">Nada aqui ainda. Coloque os arquivos em sofa\dump\iso\NOME_DO_JOGO no disco externo e importe.</p>
        )}
        {entries.map((entry) => (
          <button key={entry.path} onClick={() => openGameFilesEntry(entry)}>
            <span>{entry.name}</span>
            <span class="detail">
              {entry.main_files.length > 0 ? `${entry.main_files.length} imagem(ns) · ` : ""}
              {entry.file_count} arquivo(s) · {formatBytes(entry.size)}
            </span>
          </button>
        ))}
      </div>
      <Hints items={[["confirm", "Ver opções"], ["back", "Voltar"]]} />
    </>
  );
}

export function openGameFilesScreen(): void {
  nav.push(<GameFilesScreen />);
}
