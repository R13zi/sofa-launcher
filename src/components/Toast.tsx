import { useEffect, useState } from "preact/hooks";
import { BatteryIcon } from "./Battery";

type ToastData = { id: number } & ({ kind: "battery"; percent: number } | { kind: "download"; title: string });
type ToastContent = { kind: "battery"; percent: number } | { kind: "download"; title: string };

const TOAST_MS = 6000;
let push: ((toast: ToastContent) => void) | null = null;

/** Aviso de bateria baixa no canto, sem bloquear a tela nem tirar o foco. */
export function showBatteryToast(percent: number): void {
  push?.({ kind: "battery", percent });
}

/** Aviso de download concluído, no mesmo canto. */
export function showDownloadToast(title: string): void {
  push?.({ kind: "download", title });
}

export function BatteryToastCard({ percent }: { percent: number }) {
  return (
    <div class="toast" role="alert">
      <BatteryIcon battery={{ state: "battery", percent }} />
      <div>
        <strong>Bateria do controle baixa</strong>
        <span>{percent}% restantes. Conecte o cabo ou troque as pilhas.</span>
      </div>
    </div>
  );
}

export function DownloadToastCard({ title }: { title: string }) {
  return (
    <div class="toast ok" role="status">
      <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <circle cx="12" cy="12" r="9.5" />
        <path d="M7.5 12.5l3 3 6-6.5" />
      </svg>
      <div>
        <strong>Download concluído</strong>
        <span>{title} está pronto para jogar.</span>
      </div>
    </div>
  );
}

export function ToastHost() {
  const [toast, setToast] = useState<ToastData | null>(null);
  useEffect(() => {
    push = (content) => setToast({ id: Date.now(), ...content });
    return () => {
      push = null;
    };
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast?.id]);
  if (!toast) return null;
  return (
    <div class="toast-host" key={toast.id}>
      {toast.kind === "battery" ? <BatteryToastCard percent={toast.percent} /> : <DownloadToastCard title={toast.title} />}
    </div>
  );
}
