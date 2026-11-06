import { useEffect, useState } from "preact/hooks";
import { BatteryIcon } from "./Battery";

interface ToastData {
  id: number;
  percent: number;
}

const TOAST_MS = 6000;
let push: ((percent: number) => void) | null = null;

/** Aviso de bateria baixa no canto, sem bloquear a tela nem tirar o foco. */
export function showBatteryToast(percent: number): void {
  push?.(percent);
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

export function ToastHost() {
  const [toast, setToast] = useState<ToastData | null>(null);
  useEffect(() => {
    push = (percent) => setToast({ id: Date.now(), percent });
    return () => {
      push = null;
    };
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), TOAST_MS);
    return () => clearTimeout(timer);
  }, [toast?.id]);
  return toast ? <div class="toast-host" key={toast.id}><BatteryToastCard percent={toast.percent} /></div> : null;
}
