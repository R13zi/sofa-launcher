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

### Volume

No topo do menu rápido fica a barra de volume do Windows. Com ela em foco, esquerda e direita ajustam (segurando, o valor acelera) e A liga ou desliga o mudo. Com o mouse, clique ou arraste no trilho, ou use a roda.

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
- **`capas\`:** imagens `.jpg`, `.png` ou `.webp`, soltas com o nome do jogo ou em uma pasta com o nome dele. Em uma pasta, `capa` ou `cover` é a capa e `banner` ou `hero` é a arte grande; sem esses nomes, a primeira imagem em ordem alfabética vira a capa, e um banner nunca vira capa. As imagens são ligadas pelo nome, sem diferenciar maiúsculas, acentos e símbolos, e valem para jogos de qualquer origem. Elas têm prioridade sobre as baixadas e as da Steam, mas não sobre uma escolhida nos detalhes do jogo. O banner ideal tem 1920×620, a proporção usada pela Steam e pelo SteamGridDB; em outras proporções, a imagem é cortada para preencher a área. Para adicionar um banner a uma pasta já importada, basta colocá-lo no dump. Para trocar uma imagem, apague a antiga em `<biblioteca>\capas`, porque nada é sobrescrito.
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

## Atualizações de jogos

Nos detalhes de um jogo da Steam ou da Epic, uma linha mostra o estado dele: "Atualizado", "Atualização disponível", "Atualizando · 42% · 1,2 GB de 2,8 GB" ou, na Epic, a versão instalada. O cartão na tela inicial ganha um **↓** quando há atualização pendente ou em andamento.

- **Procurar atualizações:** na Steam, abre a Steam em segundo plano se ela estiver fechada, e ela confere sozinha. Na Epic, pede ao launcher dela para verificar.
- **Atualizar e jogar:** com atualização pendente na Steam, a tela "Iniciando" mostra o progresso real do download até o jogo abrir.

Jogos de emulador e `.exe` avulsos não têm atualização pelo Sofa.

## Instalar e desinstalar jogos

- **Aba Instalar** (última aba da tela inicial): jogos da sua conta que não estão instalados.
  - **Steam:** a lista vem da API oficial e precisa da chave da API Steam, digitada uma vez em Configurações (gratuita em steamcommunity.com/dev/apikey). Os detalhes de jogos do perfil precisam estar públicos. Ao instalar, o Sofa mostra o tamanho aproximado do jogo (dos requisitos da página da loja) e pede o disco, entre todos os discos internos; num disco sem biblioteca da Steam, ela é criada em `<disco>\SteamLibrary`. Se o jogo não cabe, o Sofa avisa quanto falta e oferece abrir o Armazenamento. Depois fecha a Steam, prepara a instalação e a reabre em segundo plano; o download começa sozinho, e o jogo aparece na biblioteca com "Instalando X%".
  - **Epic:** a lista vem do cache do launcher da Epic neste PC e pode estar incompleta. Instalar abre a janela da Epic no modo desktop, para confirmar com o cursor do controle. O cartão "Biblioteca da Epic" abre a biblioteca completa.
- **Menu → Downloads:** o que a Steam e a Epic estão baixando, com progresso, velocidade e tempo restante, mais o que está na fila e o que terminou hoje. Um download que não cabe no disco ou que parou há mais de 2 minutos vai para "Precisa de atenção", com atalho para liberar espaço. Selecionar um jogo mostra as opções: detalhes, abrir na loja ou cancelar a instalação. Pausar e reordenar a fila ficam na própria loja. Da Epic aparece só a instalação de jogos novos, com o progresso medido pela pasta do jogo; as atualizações da Epic não aparecem. A Steam só grava o progresso de vez em quando, então entre uma gravação e outra o Sofa estima pelo que a Steam recebeu da rede (com um jogo baixando por vez, como a Steam faz).
- **Download concluído:** quando um jogo termina de baixar, o Sofa toca um som curto e mostra um aviso no canto, também por cima do jogo aberto (menos em tela cheia exclusiva), mesmo com a tela de Downloads fechada.
- **Menu → Armazenamento:** espaço de cada disco e o tamanho de cada jogo, do maior para o menor. Selecionar um jogo oferece desinstalar.
- **Desinstalar** (também nos detalhes do jogo): apaga a pasta do jogo e o registro na loja. A Steam ou a Epic fecham por alguns segundos e reabrem em segundo plano. Jogos importados só são apagados se estiverem na pasta da biblioteca do Sofa.

## Bateria do controle

O nível aparece no canto da tela inicial e no topo do menu (Start ou L3 + R3). Com 20% e com 10%, um aviso aparece no canto com um som curto, inclusive por cima do jogo. Quando o controle não informa a bateria ao SDL (caso dos controles do Xbox por Bluetooth), o Sofa lê a mesma porcentagem que o Windows mostra em Configurações → Bluetooth. Sem nenhuma leitura, o ícone aparece como "Sem leitura". Para testar o aviso, escreva um número em `%APPDATA%\Sofa\fake-battery.txt` (relido a cada 30 s, inclusive com um jogo aberto). Ao apagar o arquivo, a bateria real volta em até 30 s.

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
- **Instalação silenciosa da Steam:** não é um recurso oficial da Steam; se ela mudar, pode deixar de funcionar.
- **Aviso de bateria em tela cheia exclusiva:** não aparece por cima do jogo, como o menu; o som toca mesmo assim.
- **Atualizações na Epic:** o Sofa só sabe que há atualização depois que a Epic verifica; se a atualização automática estiver desligada nela, a janela da Epic pode aparecer.
- **Steam Input:** a configuração de desktop da Steam pode interferir no controle.
