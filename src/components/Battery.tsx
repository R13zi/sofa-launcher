import { useEffect, useState } from "preact/hooks";
import { Battery, call, on } from "../api";
import { batteryFill, batteryLabel, isLow } from "./batteryFormat";

export function BatteryIcon({ battery }: { battery: Battery }) {
  const width = 15 * batteryFill(battery);
  const plugged = battery.state === "charging" || battery.state === "full" || battery.state === "wired";
  return (
    <svg class="icon battery-icon" viewBox="0 0 24 24" aria-hidden="true">
      <rect x="2" y="7" width="18" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.8" />
      <rect x="20.5" y="10" width="2" height="4" rx="0.8" fill="currentColor" />
      <rect class="battery-level" x="3.5" y="8.5" width={width} height="7" rx="1" />
      {plugged && <path d="M12.5 8.2 8.6 12.6h3l-1.1 3.4 3.9-4.4h-3z" fill="var(--bg)" />}
    </svg>
  );
}

// Um só ouvinte para todas as telas; os menus montam e desmontam o indicador várias vezes.
let latest: Battery | null = null;
const subscribers = new Set<(b: Battery | null) => void>();
let started = false;

function subscribe(fn: (b: Battery | null) => void): () => void {
  subscribers.add(fn);
  if (!started) {
    started = true;
    const publish = (b: Battery | null) => {
      latest = b;
      subscribers.forEach((s) => s(b));
    };
    void call<Battery | null>("controller_battery").then(publish).catch(() => {});
    void on<Battery | null>("battery", publish);
  }
  return () => subscribers.delete(fn);
}

/** Bateria do controle, atualizada pelos eventos do Rust. Some quando o controle não informa. */
export function BatteryIndicator({ withName }: { withName?: boolean }) {
  const [battery, setBattery] = useState<Battery | null>(latest);
  useEffect(() => {
    const unsubscribe = subscribe(setBattery);
    return () => void unsubscribe();
  }, []);
  if (!battery) return null;
  return (
    <span class={`battery ${isLow(battery) ? "low" : ""}`} title="Bateria do controle">
      <BatteryIcon battery={battery} />
      {withName && <span class="battery-name">Controle</span>}
      <span class="battery-value">{batteryLabel(battery)}</span>
    </span>
  );
}
