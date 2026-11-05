# Sofa

Launcher para jogar no PC pela TV, usando só o controle. Reúne jogos da Steam e da Epic, emuladores (Cemu, Dolphin, RPCS3 ou qualquer outro) e `.exe` avulsos.

Feito com Tauri 2, Rust, SDL3 e Preact.

## Botões

| Botão | Ação |
|---|---|
| Direcional / analógico esquerdo | Navegar |
| A | Confirmar |
| B | Voltar |
| Y | Buscar |
| LB / RB | Trocar de aba |
| Start | Menu |
| **L3 + R3 (clicar os dois analógicos), segurados por 1,5 s** | Menu rápido, inclusive com um jogo aberto |

### No teclado virtual

X apaga, Y dá espaço, LB/RB movem o cursor e Start conclui. Para acentuar, aperte o acento e depois a letra.

### No modo desktop

O modo desktop serve para janelas que exigem mouse e teclado, como logins e diálogos de emuladores. Abra pelo menu.

| Botão | Ação |
|---|---|
| Analógico esquerdo | Mover o cursor |
| Analógico direito | Rolar |
| A / X | Clique / clique direito |
| Y | Abrir o teclado |
| Direcional | Setas |
| Start / Select | Enter / Esc |
| LB / RB | Alt+Tab / Tab |
| B ou L3 + R3 | Sair |

## Mouse e teclado

O modo ativa sozinho: basta mexer o mouse ou usar uma tecla. **F2** alterna manualmente, e qualquer botão do controle volta ao modo controle.

| Tecla | Ação |
|---|---|
| Setas | Navegar |
| Enter | Confirmar |
| Esc ou Backspace | Voltar |
| F ou Ctrl + F | Buscar |
| Q / E | Trocar de aba |
| M | Menu |
| **Ctrl + Shift + F10** | Menu rápido, inclusive com um jogo aberto |

No mouse, as abas e as dicas da base da tela são clicáveis. O botão direito ou o lateral equivale a voltar, assim como clicar fora de um menu, e a roda rola a fileira de capas e as listas.

Ao digitar, o texto vem do teclado físico, com acentos. Enter conclui e Esc cancela.

## PC da sala

O Sofa inicia com o Windows por padrão. Para desativar, vá em Configurações → Iniciar com o Windows.

O início acontece depois do login, então o PC precisa entrar no Windows sem pedir senha ou PIN. Essa configuração é feita no próprio Windows. O desktop pode aparecer por um instante no boot, e um papel de parede preto disfarça isso.

Durante o jogo o Sofa fica atrás dele, então o desktop não aparece ao abrir nem ao fechar um jogo.

## Importar jogos por SSD ou pendrive

Prepare os jogos em outro computador, em uma pasta `sofa\dump` na raiz do disco:

```
X:\sofa\dump\
  cemu\      Mario Kart 8.wua
  dolphin\   Wind Waker.rvz
  rpcs3\     Demon's Souls\PS3_GAME\...
  pc\        Dusklight\dusklight.exe
             Outro Jogo\jogo.exe + sofa.json (opcional)
  iso\       Dusklight\jogo.iso
             Outro Decomp\assets\... (qualquer estrutura)
  capas\     Dusklight\capa.png
             Dusklight\banner.png
             Half-Life\banner.jpg
             Outro Jogo.png
```

Ao conectar o disco, o Sofa avisa se houver jogos novos. Também dá para abrir por Menu → Importar jogos. Para escolher onde os jogos ficam, vá em Configurações → Pasta dos jogos importados (padrão: `%USERPROFILE%\Games\Sofa`).

- **Pastas de emulador:** o nome precisa ser o id do emulador cadastrado no Sofa. Valem arquivos soltos com as extensões dele ou pastas de jogo inteiras.
- **`pc\`:** cada subpasta é um jogo e é copiada inteira, e a biblioteca aponta para o `.exe` da cópia. Um `.exe` solto em `pc\` não é importado.
- **`iso\`:** arquivos que um jogo pede depois, como em decomps. Cada subpasta é copiada inteira para `<biblioteca>\iso\<nome>`, sem filtro de extensão, seja uma ISO ou uma pasta de assets. Nada aqui vira jogo, mesmo que haja um `.exe` dentro.
- **`capas\`:** imagens `.jpg`, `.png` ou `.webp`, soltas com o nome do jogo ou em uma pasta com o nome dele. Em uma pasta, `capa` ou `cover` é a capa e `banner` ou `hero` é a arte grande; sem esses nomes, a primeira imagem em ordem alfabética vira a capa, e um banner nunca vira capa. As imagens são ligadas pelo nome, sem diferenciar maiúsculas, acentos e símbolos, e valem para jogos de qualquer origem. Elas têm prioridade sobre as baixadas e as da Steam, mas não sobre uma escolhida nos detalhes do jogo. Para adicionar um banner a uma pasta já importada, basta colocá-lo no dump. Para trocar uma imagem, apague a antiga em `<biblioteca>\capas`, porque nada é sobrescrito.
- **Qual `.exe`:** se houver um só, depois de ignorar instaladores e auxiliares, ele é usado. Se houver vários, o Sofa pergunta. Se houver só instalador, o jogo não é importado.

### Usar os arquivos em um jogo

Tudo fica em Menu → Arquivos de jogos. Se o nome da pasta for igual ao do jogo, os detalhes dele também trazem um atalho. Há duas formas de usar:

- **Usar ao iniciar:** para jogos que aceitam o arquivo por comando, como o Dusklight. O Sofa passa o caminho a cada início, e só em jogos `.exe` adicionados. O arquivo pode estar em qualquer lugar: nos detalhes do jogo, **Arquivo ao iniciar** abre o explorador, e também é lá que se troca ou remove.
- **Copiar caminho:** para os demais. Na tela do jogo, segure L3 + R3, abra o Modo desktop, aperte Y para abrir o teclado e use **Colar**.

Para já funcionar ao importar, use `{iso}` no `sofa.json`:

```json
{ "exe": "dusklight.exe", "args": ["{iso}"] }
```

Sem arquivo escolhido, o Sofa procura em `iso\` a pasta com o mesmo nome do jogo e usa a imagem, se houver uma só, ou a pasta, se só houver assets. Se não encontrar ou houver várias imagens, o argumento é retirado, e o jogo abre a própria tela de seleção. Um arquivo escolhido sempre tem prioridade. Cada jogo usa sua própria sintaxe, então `{iso}` também pode vir combinado, como `--dvd={iso}`.

### sofa.json

`sofa.json` é opcional, e `exe` e `cwd` precisam ficar dentro da pasta do jogo:

```json
{ "title": "Outro Jogo", "exe": "bin/jogo.exe", "args": ["-fullscreen"], "cwd": "bin" }
```

**O que o importador garante:**
- Só lê `sofa\dump`, sem alterar nada no disco externo.
- Nunca executa nada: um `.exe` só roda quando você aperta Jogar.
- Nunca sobrescreve arquivos: diferenças aparecem como conflito.
- Pode ser cancelado ou interrompido e retoma de onde parou.

**O que continua sendo com você:** firmware, chaves e controles de cada emulador, dependências como Visual C++ e DirectX, e atualizações do Cemu que não venham em `.wua`.

## Instalar e atualizar

Baixe o `Sofa_x.y.z_x64-setup.exe` em [Releases](https://github.com/R13zi/sofa-launcher/releases/latest). Depois de instalado, as atualizações saem por Menu → Procurar atualizações do Sofa, só com o controle.

## Publicar uma versão

1. Aumente a versão em `package.json`, `src-tauri/tauri.conf.json` e `src-tauri/Cargo.toml`.
2. Faça o commit e envie uma tag com o mesmo número:

```bash
git tag v0.3.0
```

```bash
git push origin main --tags
```

O GitHub Actions roda os testes, compila e publica o instalador.

**Chave de assinatura:** as atualizações são assinadas com uma chave privada que fica em `%USERPROFILE%\.tauri\sofa-updater.key` e nos segredos do repositório. Ela nunca vai para o código. Guarde uma cópia em lugar seguro: sem ela, as instalações existentes não aceitam novas atualizações, e seria preciso reinstalar manualmente.

## Desenvolvimento

Requisitos: Rust, Node, CMake e Visual Studio Build Tools com C++.

```bash
npm install
```

```bash
npm run tauri dev
```

```bash
npm test
```

```bash
cd src-tauri && cargo test
```

O build local precisa da chave de assinatura, porque gera também os arquivos do atualizador. No PowerShell:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\sofa-updater.key" -Raw; $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""; npm run tauri build
```

Em um navegador comum, `npm run dev` abre a interface com dados simulados e aceita o teclado: setas, Enter, Esc, `y`, `q`/`e` (abas) e `m` (menu).

Para conferir o que seria encontrado na máquina, sem gravar nada:

```bash
cd src-tauri && cargo test varredura_real -- --ignored --nocapture
```

Os dados ficam em `%APPDATA%\Sofa`.

## Limitações conhecidas

- **Pro Controller nos jogos:** jogos da Epic e `.exe` só funcionam com ele se tiverem suporte próprio. Na Steam, o Steam Input resolve.
- **Tela cheia exclusiva:** o menu não aparece sobre o jogo; o Windows troca para o launcher, como num Alt+Tab.
- **Jogos em janela:** a barra de tarefas pode aparecer na base da tela.
- **Launchers em outra pasta:** um jogo da Steam que abre um launcher instalado em outro lugar (como o da Rockstar ou da EA) não é detectado, e a sessão termina em 180 s mesmo com o jogo aberto.
- **Janelas de administrador:** o modo desktop não consegue controlá-las.
- **Steam Input:** a configuração de desktop da Steam pode interferir no controle.
