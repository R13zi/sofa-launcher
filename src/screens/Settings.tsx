import { useEffect, useState } from "preact/hooks";
import { ArtProgress, call, MonitorInfo, on, SunshineStatus } from "../api";
import { runBusy } from "../components/Busy";
import { confirmAction, openMenu, showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { Hints } from "../components/Hints";
import { openKeyboard } from "../components/VirtualKeyboard";
import { setAnimationsEnabled } from "../nav";
import { steamKeyPrompt } from "./storeActions";

/** Põe ou tira o Sofa da lista do Moonlight. O Windows pede permissão de administrador. */
function changeSunshine(register: boolean, onDone: (s: SunshineStatus) => void): void {
  const title = register ? "Adicionar o Sofa ao Moonlight" : "Remover o Sofa do Moonlight";
  confirmAction(title, register ? "Adicionar" : "Remover", () => {
    void runBusy(
      "Aguardando a permissão do Windows…",
      () => call<SunshineStatus>("sunshine_register", { register }),
      "Aceite o aviso do Windows na tela do PC. Depois o Sunshine reinicia, o que leva alguns segundos.",
    ).then((status) => {
      onDone(status);
      openMenu(register ? "O Sofa está no Moonlight" : "O Sofa saiu do Moonlight", [{ label: "OK", onClick: () => {} }],
        register ? "No Moonlight, escolha este PC e abra o item \"Sofa\"." : undefined);
    }).catch(showError);
  }, "O Windows vai pedir permissão de administrador e o Sunshine vai reiniciar: uma transmissão aberta agora cai.");
}

/** Texto do monitor escolhido; nulo é o principal do Windows. */
function monitorText(monitors: MonitorInfo[], chosen: string | null): string {
  if (!chosen) return "Automático (principal)";
  return monitors.find((m) => m.name === chosen)?.label ?? "Desconectado (usando o principal)";
}

function chooseMonitor(monitors: MonitorInfo[], chosen: string | null, onDone: (name: string | null) => void): void {
  const pick = (name: string | null) => call("monitor_set", { name }).then(() => onDone(name)).catch(showError);
  openMenu(
    "Monitor do Sofa",
    [
      { label: `${chosen === null ? "● " : ""}Automático (o principal do Windows)`, onClick: () => void pick(null) },
      ...monitors.map((m) => ({
        label: `${chosen === m.name ? "● " : ""}${m.label}${m.primary ? " (principal)" : ""}`,
        onClick: () => void pick(m.name),
      })),
    ],
    "O Sofa, o modo desktop e os avisos aparecem neste monitor. Os jogos costumam abrir no monitor principal do Windows; com \"Tela cheia sem borda\" ligada, o Sofa leva o jogo para cá.",
  );
}

export function Settings() {
  const [autostart, setAutostart] = useState(false);
  const [swap, setSwap] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [hasSteamKey, setHasSteamKey] = useState(false);
  const [libraryDir, setLibraryDir] = useState("");
  const [reduceAnimations, setReduceAnimations] = useState(false);
  const [progress, setProgress] = useState<ArtProgress | null>(null);
  const [sunshine, setSunshine] = useState<SunshineStatus | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState<string | null>(null);

  useEffect(() => {
    void call<boolean>("autostart_get").then(setAutostart);
    void call<boolean>("swap_confirm_get").then(setSwap);
    void call<boolean>("art_key_status").then(setHasKey);
    void call<boolean>("steam_key_status").then(setHasSteamKey);
    void call<string>("library_dir_get").then(setLibraryDir);
    void call<boolean>("reduce_animations_get").then(setReduceAnimations);
    void on<ArtProgress>("art-progress", setProgress);
    void call<SunshineStatus>("sunshine_status").then(setSunshine);
    void call<MonitorInfo[]>("monitors_list").then(setMonitors);
    void call<string | null>("monitor_get").then(setMonitor);
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
        <button onClick={() => chooseMonitor(monitors, monitor, setMonitor)}>
          <span>Monitor do Sofa</span><span class="detail">{monitorText(monitors, monitor)}</span>
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
        <button onClick={() => steamKeyPrompt(() => setHasSteamKey(true))}>
          <span>Chave da API Steam</span><span class="detail">{hasSteamKey ? "Configurada" : "Não configurada"}</span>
        </button>
        <p class="muted">
          Usada na aba Instalar para listar os jogos da sua conta. É gratuita em steamcommunity.com/dev/apikey (qualquer nome de
          domínio serve). Os detalhes de jogos do perfil precisam estar públicos nas configurações de privacidade da Steam.
        </p>
        {sunshine && (sunshine.installed ? (
          <button onClick={() => changeSunshine(!sunshine.registered, setSunshine)}>
            <span>{sunshine.registered ? "Remover o Sofa do Moonlight" : "Adicionar o Sofa ao Moonlight"}</span>
            <span class="detail">{sunshine.registered ? "Está na lista" : "Não está na lista"}</span>
          </button>
        ) : (
          <button disabled><span>Jogar pelo Moonlight</span><span class="detail">Sunshine não instalado</span></button>
        ))}
        {sunshine && (
          <p class="muted">
            {sunshine.installed
              ? "Com o Sofa na lista do Sunshine, ele aparece no Moonlight de outros aparelhos (TV, Xbox, celular) e abre em tela cheia neste PC."
              : "Para jogar na TV por outro aparelho com o Moonlight, instale o Sunshine neste PC (app.lizardbyte.dev) e volte aqui."}
          </p>
        )}
      </div>
      <Hints items={[["confirm", "Alterar"], ["back", "Voltar"]]} />
    </>
  );
}
