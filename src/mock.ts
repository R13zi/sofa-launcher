/** Dados simulados para testar a interface em um navegador comum, fora do Tauri. */

import type { UpdateState } from "./api";

/** Banner de teste em 1920 x 620, com marcas nas bordas para mostrar qualquer corte. */
const TEST_BANNER = `data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="620"><rect width="1920" height="620" fill="#3d6b73"/>' +
  '<rect x="6" y="6" width="1908" height="608" fill="none" stroke="#ff5a5a" stroke-width="12"/>' +
  '<circle cx="960" cy="310" r="220" fill="#e8d9a8"/></svg>',
)}`;

const games = [
  ["steam:70", "Half-Life", "steam"],
  ["steam:730", "Counter-Strike 2", "steam"],
  ["steam:242700", "The Forest", "steam"],
  ["steam:209000", "Batman: Arkham Origins", "steam"],
  ["epic:demo", "Jogo da Epic", "epic"],
  ["emu:cemu:mk8", "Mario Kart 8", "cemu"],
  ["emu:dolphin:ww", "Wind Waker", "dolphin"],
  ["manual:dusklight", "Dusklight", "manual"],
].map(([id, title, source], i) => ({
  id, title, source, art: null, hero: (i < 3 ? TEST_BANNER : null) as string | null, custom_hero: null as string | null, favorite: i === 0, hidden: false,
  last_played: i < 3 ? 1000 - i : 0, playtime_secs: i === 0 ? 5400 : 0, launch_file: null as string | null,
  update: (source === "steam" ? { state: i === 1 ? "pending" : "up_to_date" } : source === "epic" ? { state: "version", version: "1.0.4" } : null) as UpdateState | null,
}));

/** Counter-Strike 2 simula uma atualização: começa a baixar ao abrir o jogo. */
const mockDownload = { done: 0, total: 2.8 * 1024 ** 3 };
let epicVersion = "1.0.4";

const emulators = [
  { id: "cemu", name: "Cemu", kind: "cemu", exe: "C:\\Cemu\\Cemu.exe", args: ["-f", "-g", "{rom}"], rom_dirs: ["C:\\Cemu\\jogos"], exts: ["wua", "wud", "wux", "rpx"] },
];

const importItem = (id: string, title: string, status: string, pending: number, extra: object = {}) => ({
  id, title, category: id.split("/")[0], status, problem: null, size: pending, pending_bytes: pending, exe: null, exe_options: [], ...extra,
});

const GB = 1024 ** 3;
const mockImportPlans = [{
  drive: "E:\\",
  free_bytes: 200 * GB,
  library_dir: "C:\\Users\\Teste\\Games\\Sofa",
  items: [
    importItem("cemu/mario.wua", "Mario Kart 8", "new", 12 * GB),
    importItem("pc/jogo", "Jogo de PC", "needs_exe", 8 * GB, { exe_options: ["jogo.exe", "bin\\editor.exe"] }),
    importItem("iso/twilight", "Twilight Princess", "new", 4 * GB),
    importItem("capas/dusklight.png", "Dusklight", "new", 2 * 1024 * 1024),
    importItem("pc/dusk", "Dusklight", "done", 0),
    importItem("dolphin/ww.rvz", "Wind Waker", "blocked", 0, { problem: "Precisa do emulador dolphin. Cadastre-o em Menu → Emuladores." }),
    importItem("pc/conflito", "Conflito", "conflict", 0, { problem: "a.pak já existe na biblioteca com outro tamanho. Nada foi sobrescrito." }),
  ],
}];
let importCancelled = false;
let mockSteamKey = false;
let mockClipboard = "";
const mockVolume = { level: 62, muted: false };
if (typeof window !== "undefined") (window as unknown as { sofaVolume: typeof mockVolume }).sofaVolume = mockVolume;

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Listener[]>();

export function mockOn(event: string, handler: Listener): void {
  listeners.set(event, [...(listeners.get(event) ?? []), handler]);
}

export function mockEmit(event: string, payload: unknown = null): void {
  (listeners.get(event) ?? []).forEach((handler) => handler(payload));
}

// Permite simular eventos do backend pelo console do navegador.
if (typeof window !== "undefined") (window as unknown as { sofaMockEmit: typeof mockEmit }).sofaMockEmit = mockEmit;

let session: { game_id: string; title: string; art: null; hero: string | null; phase: string } | null = null;

function setSession(next: typeof session): void {
  session = next;
  mockEmit("session", next);
}

export function mockInvoke(command: string, args: Record<string, unknown>): unknown {
  console.debug("[mock]", command, args);
  switch (command) {
    case "library_list": return games;
    case "emulators_list": return emulators;
    case "session_info": return session;
    case "launch_game": {
      const game = games.find((g) => g.id === args.id);
      if (!game) throw new Error("Jogo não encontrado");
      const started = { game_id: game.id, title: game.title, art: null, hero: game.hero, phase: "launching" };
      setSession(started);
      const updating = game.update?.state === "pending";
      if (updating) {
        // Baixa em cerca de 6 s, como a Steam faria antes de abrir o jogo.
        game.update = { state: "downloading", done: 0, total: mockDownload.total };
        const tick = setInterval(() => {
          mockDownload.done = Math.min(mockDownload.total, mockDownload.done + mockDownload.total / 12);
          game.update = mockDownload.done >= mockDownload.total
            ? { state: "up_to_date" }
            : { state: "downloading", done: mockDownload.done, total: mockDownload.total };
          if (game.update.state === "up_to_date") clearInterval(tick);
        }, 500);
      }
      setTimeout(() => {
        if (session === started) setSession({ ...started, phase: "playing" });
      }, updating ? 6800 : 1500);
      return null;
    }
    case "game_update_status": {
      const game = games.find((g) => g.id === args.id);
      if (game?.source === "epic") return { state: "version", version: epicVersion };
      return game?.update ?? null;
    }
    case "game_update_check": {
      const game = games.find((g) => g.id === args.id);
      if (game?.source === "epic") setTimeout(() => (epicVersion = "1.0.5"), 2000);
      return game?.source === "steam";
    }
    case "session_cancel": case "session_close": setSession(null); return null;
    case "reduce_animations_get": return false;
    case "set_hero": {
      const game = games.find((g) => g.id === args.id);
      if (game) {
        game.custom_hero = (args.path as string | null) ?? null;
        game.hero = game.custom_hero;
      }
      return null;
    }
    case "set_launch_file": {
      const game = games.find((g) => g.id === args.id);
      if (game) game.launch_file = (args.path as string | null) ?? null;
      return null;
    }
    case "update_check": return { current: "0.2.0", version: "0.3.0", notes: "Notas da versão simulada." };
    case "update_install":
      return new Promise((resolve) => {
        let done = 0;
        const timer = setInterval(() => {
          done += 25;
          mockEmit("update-progress", { done: done * 1024 * 1024, total: 100 * 1024 * 1024 });
          if (done >= 100) {
            clearInterval(timer);
            resolve(null);
          }
        }, 100);
      });
    case "library_dir_get": return "C:\\Users\\Teste\\Games\\Sofa";
    case "game_files_list": return [
      { name: "Dusklight", path: "C:\\Games\\Sofa\\iso\\Dusklight", size: 4 * GB, file_count: 2, main_files: ["C:\\Games\\Sofa\\iso\\Dusklight\\disco\\jogo.iso"] },
      { name: "Mario 64", path: "C:\\Games\\Sofa\\iso\\Mario 64", size: GB, file_count: 3200, main_files: [] },
    ];
    case "controller_battery": return { state: "battery", percent: 64 };
    case "storage_overview": return [
      {
        root: "C:\\", total: 476 * GB, free: 92 * GB,
        games: games.filter((g) => g.source === "steam").map((g, i) => ({ id: g.id, title: g.title, source: g.source, art: null, size: (40 - i * 9) * GB, removable: true })),
      },
      { root: "D:\\", total: 931 * GB, free: 610 * GB, games: [{ id: "manual:dusklight", title: "Dusklight", source: "manual", art: null, size: 3 * GB, removable: true }] },
    ];
    case "game_size": {
      const game = games.find((g) => g.id === args.id);
      return game ? { id: game.id, title: game.title, source: game.source, art: null, size: 12.3 * GB, removable: game.source !== "cemu" } : null;
    }
    case "game_uninstall":
      return new Promise((resolve) => setTimeout(() => {
        const index = games.findIndex((g) => g.id === args.id);
        if (index >= 0) games.splice(index, 1);
        mockEmit("library-changed");
        resolve(null);
      }, 1500));
    case "steam_key_status": return mockSteamKey;
    case "steam_key_save": mockSteamKey = true; return null;
    case "store_installable": return {
      steam: mockSteamKey ? [
        { id: "steam:620", title: "Portal 2", source: "steam", art: null },
        { id: "steam:400", title: "Portal", source: "steam", art: null },
      ] : [],
      steam_error: null,
      steam_needs_key: !mockSteamKey,
      epic: [{ id: "epic:Calluna", title: "Control", source: "epic", art: null }],
    };
    case "steam_install_targets": return new Promise((resolve) => setTimeout(() => resolve({
      // Portal pede mais do que cabe no C:, para testar o aviso de falta de espaço.
      size: args.appid === "400" ? 120 * GB : args.appid === "620" ? 13 * GB : null,
      targets: [
        { path: "C:\\Program Files (x86)\\Steam", free: 92 * GB, total: 476 * GB, new: false },
        { path: "D:\\SteamLibrary", free: 610 * GB, total: 931 * GB, new: true },
      ],
    }), 600));
    case "steam_install":
      return new Promise((resolve) => setTimeout(() => {
        games.push({
          id: `steam:${args.appid}`, title: args.name as string, source: "steam", art: null, hero: null, custom_hero: null, favorite: false,
          hidden: false, last_played: 0, playtime_secs: 0, launch_file: null, update: { state: "installing", done: GB, total: 8 * GB },
        });
        mockEmit("library-changed");
        resolve(null);
      }, 2000));
    case "volume_get": return { ...mockVolume };
    case "volume_set": mockVolume.level = args.level as number; return null;
    case "volume_set_mute": mockVolume.muted = args.muted as boolean; return null;
    case "clipboard_copy": mockClipboard = args.text as string; (window as unknown as { sofaClipboard: string }).sofaClipboard = mockClipboard; return null;
    case "import_scan": return mockImportPlans;
    case "import_cancel": importCancelled = true; return null;
    case "import_start": {
      importCancelled = false;
      const ids = args.ids as string[];
      const items = mockImportPlans[0].items.filter((i) => ids.includes(i.id));
      const total = items.reduce((sum, i) => sum + i.pending_bytes, 0);
      let step = 0;
      const base = { total_bytes: total, bytes_per_sec: 50 * 1024 * 1024, item_count: items.length, cancelled: false, imported: [], errors: [] };
      const timer = setInterval(() => {
        step++;
        const done = importCancelled || step >= 4;
        if (done) clearInterval(timer);
        mockEmit("import-progress", {
          ...base,
          done_bytes: (total * step) / 4,
          current: items[0]?.title ?? "",
          item_index: 1,
          finished: done,
          cancelled: importCancelled,
          imported: done && !importCancelled ? items.map((i) => i.title) : [],
        });
      }, 150);
      return null;
    }
    case "art_key_status": case "autostart_get": case "swap_confirm_get": return false;
    case "fs_roots": return [{ name: "Documentos", path: "C:\\Docs", dir: true }, { name: "Disco C:", path: "C:\\", dir: true }];
    case "fs_list": return [{ name: "Jogos", path: `${args.path}\\Jogos`, dir: true }, { name: "jogo.exe", path: `${args.path}\\jogo.exe`, dir: false }];
    case "set_favorite": {
      const game = games.find((g) => g.id === args.id);
      if (game) game.favorite = args.value as boolean;
      return null;
    }
    default: return null;
  }
}
