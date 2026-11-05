import { useEffect, useState } from "preact/hooks";
import { ArtProgress, call, on } from "../api";
import { showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { Hints } from "../components/Hints";
import { openKeyboard } from "../components/VirtualKeyboard";
import { setAnimationsEnabled } from "../nav";

export function Settings() {
  const [autostart, setAutostart] = useState(false);
  const [swap, setSwap] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [libraryDir, setLibraryDir] = useState("");
  const [reduceAnimations, setReduceAnimations] = useState(false);
  const [progress, setProgress] = useState<ArtProgress | null>(null);

  useEffect(() => {
    void call<boolean>("autostart_get").then(setAutostart);
    void call<boolean>("swap_confirm_get").then(setSwap);
    void call<boolean>("art_key_status").then(setHasKey);
    void call<string>("library_dir_get").then(setLibraryDir);
    void call<boolean>("reduce_animations_get").then(setReduceAnimations);
    void on<ArtProgress>("art-progress", setProgress);
  }, []);

  return (
    <>
      <div class="top-bar"><h1>Configurações</h1></div>
      <div class="scroll list">
        <button
          data-autofocus
          onClick={() => call("autostart_set", { enabled: !autostart }).then(() => setAutostart(!autostart)).catch(showError)}
        >
          <span>Iniciar com o Windows</span><span class="detail">{autostart ? "Ligado" : "Desligado"}</span>
        </button>
        <button onClick={() => call("swap_confirm_set", { value: !swap }).then(() => setSwap(!swap)).catch(showError)}>
          <span>Inverter confirmar e voltar (A/B)</span><span class="detail">{swap ? "Ligado" : "Desligado"}</span>
        </button>
        <button
          onClick={() => call("reduce_animations_set", { value: !reduceAnimations }).then(() => {
            setAnimationsEnabled(reduceAnimations);
            setReduceAnimations(!reduceAnimations);
          }).catch(showError)}
        >
          <span>Animações</span><span class="detail">{reduceAnimations ? "Desligado" : "Ligado"}</span>
        </button>
        <button
          onClick={() => openFileBrowser({
            title: "Pasta para jogos importados",
            pickDir: true,
            onPick: (path) => call("library_dir_set", { path }).then(() => setLibraryDir(path)).catch(showError),
          })}
        >
          <span>Pasta dos jogos importados</span><span class="detail">{libraryDir}</span>
        </button>
        <button
          onClick={() => openKeyboard({
            title: "Chave da API do SteamGridDB",
            secret: true,
            onDone: (key) => {
              if (!key.trim()) return;
              call("art_key_save", { key }).then(() => setHasKey(true)).catch(showError);
            },
          })}
        >
          <span>Chave do SteamGridDB</span><span class="detail">{hasKey ? "Configurada" : "Não configurada"}</span>
        </button>
        <button disabled={!hasKey} onClick={() => call("art_fetch").catch(showError)}>
          Baixar capas e banners que faltam
        </button>
        {progress && (
          <p class={progress.error ? "error" : "muted"}>
            {progress.finished ? "Concluído: " : "Baixando: "}
            {progress.done}/{progress.total}, {progress.found} imagem(ns) encontrada(s)
            {progress.error ? `. Último erro: ${progress.error}` : ""}
          </p>
        )}
        <p class="muted">
          A chave é gratuita em steamgriddb.com, em Preferences → API, e fica guardada no Gerenciador de Credenciais do Windows.
        </p>
      </div>
      <Hints items={[["confirm", "Alterar"], ["back", "Voltar"]]} />
    </>
  );
}
