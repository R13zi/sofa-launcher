import { useEffect, useRef, useState } from "preact/hooks";
import { call, VolumeState } from "../api";
import { applyStep, clampLevel, HoldState, levelFromPointer } from "./volumeLogic";

const STEP_EVENT = "sofa-step";

/**
 * Se o foco está numa barra, manda esquerda ou direita para ela. Devolve `false` quando não
 * está, para a navegação normal seguir.
 */
export function stepFocusedSlider(dir: -1 | 1): boolean {
  const el = document.activeElement as HTMLElement | null;
  if (!el?.matches("[data-slider]")) return false;
  el.dispatchEvent(new CustomEvent(STEP_EVENT, { detail: dir }));
  return true;
}

/** Envia ao Windows só o valor mais recente, sem acumular chamadas enquanto uma está em andamento. */
function useLatestSender(send: (level: number) => Promise<unknown>, onFail: () => void) {
  const busy = useRef(false);
  const pending = useRef<number | null>(null);
  const flush = () => {
    if (busy.current || pending.current === null) return;
    const level = pending.current;
    pending.current = null;
    busy.current = true;
    send(level).catch(onFail).finally(() => {
      busy.current = false;
      flush();
    });
  };
  return (level: number) => {
    pending.current = level;
    flush();
  };
}

const SpeakerIcon = ({ level, muted }: VolumeState) => (
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M4 9v6h4l5 4V5L8 9z" />
    {muted || level === 0 ? (
      <path d="M17 9l5 6M22 9l-5 6" />
    ) : (
      <>
        <path d="M16 9.5a3.5 3.5 0 0 1 0 5" />
        {level >= 40 && <path d="M18.5 7a7 7 0 0 1 0 10" />}
      </>
    )}
  </svg>
);

export function VolumeBar({ initial }: { initial: VolumeState | null }) {
  const [state, setState] = useState<VolumeState | null>(initial);
  const stateRef = useRef(state);
  stateRef.current = state;
  const hold = useRef<HoldState | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);

  const unavailable = () => setState(null);
  const sendLevel = useLatestSender((level) => call("volume_set", { level }), unavailable);

  const setLevel = (level: number) => {
    const current = stateRef.current;
    if (!current) return;
    if (current.muted) void call("volume_set_mute", { muted: false }).catch(unavailable);
    setState({ level, muted: false });
    sendLevel(level);
  };

  useEffect(() => {
    const el = button.current;
    if (!el) return;
    const onStep = (e: Event) => {
      const current = stateRef.current;
      if (!current) return;
      const result = applyStep(current.level, (e as CustomEvent<-1 | 1>).detail, performance.now(), hold.current);
      if (!result) return;
      hold.current = result.hold;
      if (result.level !== current.level || current.muted) setLevel(result.level);
    };
    el.addEventListener(STEP_EVENT, onStep);
    return () => el.removeEventListener(STEP_EVENT, onStep);
  }, [state === null]);

  if (!state) {
    return (
      <button class="volume-bar" disabled>
        <SpeakerIcon level={0} muted />
        <span class="volume-label">Sem saída de áudio</span>
      </button>
    );
  }

  const fromPointer = (e: PointerEvent) => {
    const rect = track.current!.getBoundingClientRect();
    setLevel(levelFromPointer(e.clientX, rect.left, rect.width));
  };

  return (
    <button
      ref={button}
      class={`volume-bar ${state.muted ? "muted" : ""}`}
      data-slider
      aria-label={`Volume ${state.level}${state.muted ? ", mudo" : ""}`}
      onClick={() => {
        // Um clique que terminou de arrastar no trilho não deve mudar o mudo.
        if (dragging.current) return;
        const muted = !state.muted;
        setState({ ...state, muted });
        void call("volume_set_mute", { muted }).catch(unavailable);
      }}
      onWheel={(e) => {
        e.preventDefault();
        button.current?.focus();
        setLevel(clampLevel(state.level + (e.deltaY < 0 ? 2 : -2)));
      }}
    >
      <SpeakerIcon {...state} />
      <span class="volume-label">Volume</span>
      <div
        ref={track}
        class="volume-track"
        onPointerDown={(e) => {
          dragging.current = true;
          (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
          fromPointer(e);
        }}
        onPointerMove={(e) => {
          if (dragging.current) fromPointer(e);
        }}
        onPointerUp={() => {
          setTimeout(() => (dragging.current = false), 0);
        }}
      >
        <div class="volume-fill" style={{ width: `${state.level}%` }} />
        <div class="volume-knob" style={{ left: `${state.level}%` }} />
      </div>
      <span class="volume-value">{state.muted ? "Mudo" : state.level}</span>
    </button>
  );
}
