import { useEffect, useState } from "preact/hooks";
import { call, Emulator } from "../api";
import { confirmAction, openMenu, showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { Hints } from "../components/Hints";
import { openKeyboard } from "../components/VirtualKeyboard";
import { nav } from "../nav";
import { refreshLibrary } from "./QuickMenu";

const PRESETS = [
  ["cemu", "Cemu (Wii U)"],
  ["dolphin", "Dolphin (GameCube/Wii)"],
  ["rpcs3", "RPCS3 (PS3)"],
  ["generic", "Outro emulador"],
] as const;

/** Atributos separados por espaço; `{rom}` indica onde entra o caminho do jogo. */
export const parseArgs = (text: string) => text.split(" ").filter(Boolean);
export const parseExts = (text: string) =>
  text.split(/[\s,]+/).map((e) => e.replace(/^\./, "").toLowerCase()).filter(Boolean);

function EmulatorEdit({ id, reload }: { id: string; reload: () => void }) {
  const [emu, setEmu] = useState<Emulator | null>(null);

  const load = async () => {
    const list = await call<Emulator[]>("emulators_list");
    setEmu(list.find((e) => e.id === id) ?? null);
  };
  useEffect(() => void load(), []);

  if (!emu) return <p class="muted">Carregando…</p>;

  const save = async (patch: Partial<Emulator>) => {
    try {
      await call("emulator_update", { emulator: { ...emu, ...patch } });
      await load();
      await refreshLibrary();
      reload();
    } catch (e) {
      showError(e);
    }
  };

  return (
    <>
      <div class="top-bar"><h1>{emu.name}</h1></div>
      <div class="scroll list">
        <button
          data-autofocus
          onClick={() => openFileBrowser({ title: "Executável do emulador", exts: ["exe"], onPick: (exe) => save({ exe }) })}
        >
          <span>Executável</span><span class="detail">{emu.exe}</span>
        </button>
        <button
          onClick={() => openKeyboard({
            title: "Argumentos ({rom} é o jogo)",
            initial: emu.args.join(" "),
            onDone: (text) => save({ args: parseArgs(text) }),
          })}
        >
          <span>Argumentos</span><span class="detail">{emu.args.join(" ")}</span>
        </button>
        {emu.kind !== "rpcs3" && (
          <button
            onClick={() => openKeyboard({
              title: "Extensões separadas por vírgula",
              initial: emu.exts.join(", "),
              onDone: (text) => save({ exts: parseExts(text) }),
            })}
          >
            <span>Extensões</span><span class="detail">{emu.exts.join(", ") || "nenhuma"}</span>
          </button>
        )}
        <button
          onClick={() => openFileBrowser({
            title: "Pasta de jogos",
            pickDir: true,
            onPick: (dir) => save({ rom_dirs: [...emu.rom_dirs.filter((d) => d !== dir), dir] }),
          })}
        >
          + Adicionar pasta de jogos
        </button>
        {emu.rom_dirs.map((dir) => (
          <button
            key={dir}
            onClick={() => confirmAction("Remover esta pasta da varredura?", "Remover", () => save({ rom_dirs: emu.rom_dirs.filter((d) => d !== dir) }), dir)}
          >
            <span>Pasta</span><span class="detail">{dir}</span>
          </button>
        ))}
        <button
          class="danger"
          onClick={() => confirmAction(`Remover ${emu.name}?`, "Remover", async () => {
            try {
              await call("emulator_remove", { id: emu.id });
              await refreshLibrary();
              reload();
              nav.pop();
            } catch (e) {
              showError(e);
            }
          }, "Só tira o emulador do Sofa. Nenhum arquivo é apagado.")}
        >
          Remover emulador
        </button>
      </div>
      <Hints items={[["confirm", "Selecionar"], ["back", "Voltar"]]} />
    </>
  );
}

export function Emulators() {
  const [list, setList] = useState<Emulator[] | null>(null);
  const load = () => call<Emulator[]>("emulators_list").then(setList).catch(showError);
  useEffect(() => void load(), []);

  if (!list) return <p class="muted">Carregando…</p>;

  const add = () => openMenu("Qual emulador?", PRESETS.map(([kind, label]) => ({
    label,
    onClick: () => openFileBrowser({
      title: `Executável: ${label}`,
      exts: ["exe"],
      onPick: async (exe) => {
        try {
          const emu = await call<Emulator>("emulator_add", { kind, exe });
          await load();
          nav.push(<EmulatorEdit id={emu.id} reload={load} />);
        } catch (e) {
          showError(e);
        }
      },
    }),
  })));

  return (
    <>
      <div class="top-bar"><h1>Emuladores</h1></div>
      <div class="scroll list">
        {list.map((emu) => (
          <button key={emu.id} onClick={() => nav.push(<EmulatorEdit id={emu.id} reload={load} />)}>
            <span>{emu.name}</span><span class="detail">{emu.exe}</span>
          </button>
        ))}
        <button data-autofocus={list.length === 0 ? "" : undefined} onClick={add}>+ Adicionar emulador</button>
      </div>
      <Hints items={[["confirm", "Selecionar"], ["back", "Voltar"]]} />
    </>
  );
}
