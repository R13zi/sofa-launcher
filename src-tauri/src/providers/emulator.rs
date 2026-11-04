//! Emuladores: predefinições para Cemu, Dolphin e RPCS3, e um modelo genérico.

use super::{manual::cwd_of, Game, Launch};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Emulator {
    pub id: String,
    pub name: String,
    /// "cemu", "dolphin", "rpcs3" ou "generic".
    pub kind: String,
    pub exe: String,
    /// Argumentos, com `{rom}` no lugar do jogo.
    pub args: Vec<String>,
    pub rom_dirs: Vec<String>,
    pub exts: Vec<String>,
}

pub fn preset(kind: &str, exe: &str) -> Emulator {
    let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let (name, args, exts) = match kind {
        "cemu" => ("Cemu", strs(&["-f", "-g", "{rom}"]), strs(&["wua", "wud", "wux", "rpx"])),
        "dolphin" => ("Dolphin", strs(&["-b", "-e", "{rom}"]), strs(&["iso", "rvz", "gcm", "wbfs", "gcz", "ciso", "wad"])),
        "rpcs3" => ("RPCS3", strs(&["--no-gui", "{rom}"]), vec![]),
        _ => ("Emulador", strs(&["{rom}"]), vec![]),
    };
    Emulator {
        id: kind.to_string(),
        name: name.into(),
        kind: kind.into(),
        exe: exe.into(),
        args,
        rom_dirs: vec![],
        exts,
    }
}

pub fn build_args(template: &[String], rom: &str) -> Vec<String> {
    template.iter().map(|a| a.replace("{rom}", rom)).collect()
}

/// "Zelda (USA) [v16]" → "Zelda".
pub fn clean_title(stem: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in stem.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(if c == '_' { ' ' } else { c }),
            _ => {}
        }
    }
    let trimmed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.is_empty() {
        stem.to_string()
    } else {
        trimmed
    }
}

/// Lê um título de um PARAM.SFO do PS3.
pub fn parse_sfo(data: &[u8], wanted: &str) -> Option<String> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(data.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    if data.get(0..4)? != b"\0PSF" {
        return None;
    }
    let key_table = u32_at(8)? as usize;
    let data_table = u32_at(12)? as usize;
    let count = u32_at(16)? as usize;
    for i in 0..count {
        let entry = 20 + i * 16;
        let key_start = key_table + u16_at(entry)? as usize;
        let len = u32_at(entry + 4)? as usize;
        let value_start = data_table + u32_at(entry + 12)? as usize;
        let key_bytes = data.get(key_start..)?;
        let key_end = key_bytes.iter().position(|&b| b == 0)?;
        if &key_bytes[..key_end] == wanted.as_bytes() {
            let raw = data.get(value_start..value_start + len)?;
            let text = String::from_utf8_lossy(raw);
            return Some(text.trim_end_matches('\0').to_string());
        }
    }
    None
}

/// Pastas de jogos configuradas dentro do Cemu (settings.xml).
pub fn cemu_game_paths(xml: &str) -> Vec<String> {
    let Some(start) = xml.find("<GamePaths>") else { return vec![] };
    let Some(end) = xml[start..].find("</GamePaths>") else { return vec![] };
    let section = &xml[start..start + end];
    section
        .split("<Entry>")
        .skip(1)
        .filter_map(|s| s.split("</Entry>").next())
        .map(|s| s.replace("&amp;", "&"))
        .collect()
}

/// Pastas de jogos configuradas dentro do Dolphin (Dolphin.ini).
pub fn dolphin_iso_paths(ini: &str) -> Vec<String> {
    ini.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            let is_path = key.starts_with("ISOPath") && key[7..].chars().all(|c| c.is_ascii_digit()) && key.len() > 7;
            is_path.then(|| value.trim().to_string())
        })
        .filter(|v| !v.is_empty())
        .collect()
}

/// Lista de jogos do RPCS3 (games.yml), no formato `SERIAL: caminho`.
pub fn rpcs3_game_paths(yml: &str) -> Vec<String> {
    yml.lines()
        .filter_map(|line| line.split_once(": ").map(|(_, v)| v.trim().trim_matches('"').to_string()))
        .filter(|v| !v.is_empty())
        .collect()
}

fn walk_files(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                walk_files(&path, depth - 1, out);
            }
        } else {
            out.push(path);
        }
    }
}

fn own_dirs(emu: &Emulator) -> Vec<String> {
    let exe_dir = Path::new(&emu.exe).parent().unwrap_or(Path::new("")).to_path_buf();
    match emu.kind.as_str() {
        "cemu" => std::fs::read_to_string(exe_dir.join("settings.xml")).map(|t| cemu_game_paths(&t)).unwrap_or_default(),
        "dolphin" => {
            let mut candidates = vec![exe_dir.join(r"User\Config\Dolphin.ini")];
            if let Ok(profile) = std::env::var("USERPROFILE") {
                candidates.push(Path::new(&profile).join(r"Documents\Dolphin Emulator\Config\Dolphin.ini"));
            }
            candidates.iter().filter_map(|p| std::fs::read_to_string(p).ok()).flat_map(|t| dolphin_iso_paths(&t)).collect()
        }
        _ => vec![],
    }
}

fn make_game(emu: &Emulator, title: String, rom: &str) -> Game {
    Game {
        id: format!("emu:{}:{}", emu.id, rom.to_lowercase()),
        title,
        source: emu.id.clone(),
        launch: Launch::Exec { path: emu.exe.clone(), args: build_args(&emu.args, rom), cwd: cwd_of(&emu.exe) },
        art: None,
    }
}

fn scan_files(emu: &Emulator) -> Vec<Game> {
    let exts: Vec<String> = emu.exts.iter().map(|e| e.trim_start_matches('.').to_lowercase()).collect();
    let mut dirs = emu.rom_dirs.clone();
    dirs.extend(own_dirs(emu));
    dirs.sort();
    dirs.dedup();

    let mut games = vec![];
    for dir in dirs {
        let mut files = vec![];
        walk_files(Path::new(&dir), 4, &mut files);
        for file in files {
            let ext = file.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !exts.contains(&ext) {
                continue;
            }
            // Jogo do Wii U descompactado: o título é a pasta acima de `code`.
            let title = if ext == "rpx" {
                let parent = file.parent();
                if !parent.and_then(Path::file_name).is_some_and(|n| n.eq_ignore_ascii_case("code")) {
                    continue;
                }
                parent.and_then(Path::parent).and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned())
            } else {
                file.file_stem().map(|n| n.to_string_lossy().into_owned())
            };
            let Some(title) = title else { continue };
            games.push(make_game(emu, clean_title(&title), &file.to_string_lossy()));
        }
    }
    games
}

/// Devolve (EBOOT.BIN, PARAM.SFO) de uma pasta de jogo do PS3, em formato de disco ou HDD.
pub fn ps3_layout(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    for base in [dir.join("PS3_GAME"), dir.to_path_buf()] {
        let eboot = base.join("USRDIR").join("EBOOT.BIN");
        let sfo = base.join("PARAM.SFO");
        if eboot.exists() && sfo.exists() {
            return Some((eboot, sfo));
        }
    }
    None
}

fn scan_rpcs3(emu: &Emulator) -> Vec<Game> {
    let exe_dir = Path::new(&emu.exe).parent().unwrap_or(Path::new("")).to_path_buf();
    let mut candidates: Vec<PathBuf> = vec![];
    for file in [exe_dir.join(r"config\games.yml"), exe_dir.join("games.yml")] {
        if let Ok(text) = std::fs::read_to_string(file) {
            candidates.extend(rpcs3_game_paths(&text).into_iter().map(PathBuf::from));
        }
    }
    let mut parents: Vec<PathBuf> = emu.rom_dirs.iter().map(PathBuf::from).collect();
    parents.push(exe_dir.join(r"dev_hdd0\game"));
    for parent in parents {
        candidates.push(parent.clone());
        if let Ok(entries) = std::fs::read_dir(parent) {
            candidates.extend(entries.flatten().map(|e| e.path()));
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut games = vec![];
    for dir in candidates {
        let Some((eboot, sfo)) = ps3_layout(&dir) else { continue };
        let rom = eboot.to_string_lossy().into_owned();
        if !seen.insert(rom.to_lowercase()) {
            continue;
        }
        let data = std::fs::read(sfo).unwrap_or_default();
        // GD são atualizações instaladas, não jogos.
        if parse_sfo(&data, "CATEGORY").as_deref() == Some("GD") {
            continue;
        }
        let title = parse_sfo(&data, "TITLE")
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Jogo de PS3".into());
        games.push(make_game(emu, title, &rom));
    }
    games
}

pub fn scan(emu: &Emulator) -> Vec<Game> {
    if emu.kind == "rpcs3" {
        scan_rpcs3(emu)
    } else {
        scan_files(emu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monta_argumentos_com_espacos_intactos() {
        let emu = preset("cemu", r"C:\Cemu\Cemu.exe");
        assert_eq!(build_args(&emu.args, r"D:\Jogos\Mario Kart 8.wua"), vec!["-f", "-g", r"D:\Jogos\Mario Kart 8.wua"]);
    }

    #[test]
    fn limpa_titulos() {
        assert_eq!(clean_title("Zelda_Wind_Waker (USA) [v16]"), "Zelda Wind Waker");
        assert_eq!(clean_title("(USA)"), "(USA)");
    }

    #[test]
    fn le_pastas_do_cemu() {
        let xml = "<content><GamePaths><Entry>D:\\Wii U</Entry><Entry>E:\\A &amp; B</Entry></GamePaths><Entry>nao</Entry></content>";
        assert_eq!(cemu_game_paths(xml), vec![r"D:\Wii U", r"E:\A & B"]);
    }

    #[test]
    fn le_pastas_do_dolphin() {
        let ini = "[General]\nISOPaths = 2\nISOPath0 = D:\\GC\nISOPath1 = \nRecursiveISOPaths = True\n";
        assert_eq!(dolphin_iso_paths(ini), vec![r"D:\GC"]);
    }

    #[test]
    fn le_games_yml() {
        let yml = "BLUS30443: D:/PS3/Demons Souls/\nBLES00000: \"E:/PS3/Outro\"\n";
        assert_eq!(rpcs3_game_paths(yml), vec!["D:/PS3/Demons Souls/", "E:/PS3/Outro"]);
    }

    fn sfo() -> Vec<u8> {
        let keys = b"CATEGORY\0TITLE\0\0";
        let values = b"DG\0\0Demon's Souls\0\0\0";
        let key_table = 20 + 2 * 16;
        let data_table = key_table + keys.len();
        let mut out = b"\0PSF".to_vec();
        out.extend(0x0101u32.to_le_bytes());
        out.extend((key_table as u32).to_le_bytes());
        out.extend((data_table as u32).to_le_bytes());
        out.extend(2u32.to_le_bytes());
        for (key_off, len, data_off) in [(0u16, 3u32, 0u32), (9, 14, 4)] {
            out.extend(key_off.to_le_bytes());
            out.extend(0x0204u16.to_le_bytes());
            out.extend(len.to_le_bytes());
            out.extend(len.to_le_bytes());
            out.extend(data_off.to_le_bytes());
        }
        out.extend(keys);
        out.extend(values);
        out
    }

    #[test]
    fn le_param_sfo() {
        let data = sfo();
        assert_eq!(parse_sfo(&data, "TITLE").as_deref(), Some("Demon's Souls"));
        assert_eq!(parse_sfo(&data, "CATEGORY").as_deref(), Some("DG"));
        assert_eq!(parse_sfo(&data, "NADA"), None);
        assert_eq!(parse_sfo(b"lixo", "TITLE"), None);
    }

    #[test]
    fn varre_pasta_filtrando_extensoes_e_rpx_fora_de_code() {
        let root = std::env::temp_dir().join(format!("sofa-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(r"Mario (USA)\code")).unwrap();
        std::fs::create_dir_all(root.join("outra")).unwrap();
        std::fs::write(root.join(r"Mario (USA)\code\game.rpx"), b"").unwrap();
        std::fs::write(root.join(r"outra\solto.rpx"), b"").unwrap();
        std::fs::write(root.join("Zelda [v1].WUA"), b"").unwrap();
        std::fs::write(root.join("leia.txt"), b"").unwrap();

        let mut emu = preset("cemu", r"C:\nao-existe\Cemu.exe");
        emu.rom_dirs = vec![root.to_string_lossy().into_owned()];
        let mut titles: Vec<String> = scan(&emu).into_iter().map(|g| g.title).collect();
        titles.sort();
        assert_eq!(titles, vec!["Mario", "Zelda"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
