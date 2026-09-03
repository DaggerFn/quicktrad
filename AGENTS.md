# AGENTS.md — guia pra IA/agente mexendo neste repo

Leia isto antes de compilar, rodar ou "corrigir" algo aqui. Cada item abaixo
é um bug real que já mordeu — a causa não é óbvia pelo erro na tela, então
não repita a investigação do zero.

## 1. NUNCA rode `cargo build` direto em `src-tauri/`

Use sempre `npm run tauri build` (produção) ou `npm run tauri dev`
(desenvolvimento), a partir da raiz do repo.

**Por quê:** o binário decide em tempo de compilação se carrega o frontend
embutido (`frontendDist`) ou o servidor de dev (`devUrl`,
`http://localhost:1420`). Essa decisão é sinalizada pela CLI `tauri` via env
var (`TAURI_ENV_DEBUG`), não só pelo profile do cargo. Um `cargo build
--release` direto ignora esse sinal e o binário sobe tentando abrir
`localhost:1420` — like não tem servidor rodando, a janela mostra **"Could
not connect to localhost: Connection refused"**.

Se editar só código Rust e quiser rebuild rápido sem rodar `npm run build`
de novo (frontend não mudou): `npm run tauri build` ainda é o comando certo,
ele só re-roda o `beforeBuildCommand` se necessário — não tem atalho seguro
via cargo puro.

## 2. Linux + Wayland nativo (KDE Plasma, GNOME, Hyprland, etc.): dois bugs de plataforma, já corrigidos em `main.rs`

`src-tauri/src/main.rs` já seta duas env vars no início do `main()`,
condicionadas a `target_os = "linux"` e só se o usuário não tiver definido a
própria (`var_os(..).is_none()`) — não desfaça isso sem entender por quê:

- **`GDK_BACKEND=x11`** — sem isso, GTK3/webkit2gtk crasham com
  `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display`
  assim que a janela tenta aparecer, sob Wayland nativo. Forçar XWayland
  evita o crash. Testado quebrando e sendo corrigido no KDE Plasma 6/Wayland;
  a causa é uma chamada X11-específica dentro do stack GTK3 que o Tauri usa
  no Linux — não é exclusivo do KDE, deve valer pra GNOME/Hyprland também.
- **`WEBKIT_DISABLE_DMABUF_RENDERER=1`** — sem isso, o webview abre em
  **branco/preto, sem nenhum erro no log** (bug conhecido do webkit2gtk
  >= 2.42 com o renderer via DMA-BUF/GBM em várias combinações de driver
  gráfico + compositor). Only sintoma visível nos logs antes do fix: linhas
  tipo `Failed to create GBM buffer of size WxH: Invalid argument` — se você
  ver essas linhas, é este bug.

Se investigar um bug visual/crash novo no Linux, **rode o binário no
terminal primeiro** (`quicktrad` sem redirecionar stderr) e leia a saída
antes de mexer em código — os dois bugs acima só ficaram óbvios assim.

## 3. Esta distro (Arch/CachyOS) não usa os bundles gerados

`npm run tauri build` gera `.deb`/`.rpm` em `src-tauri/target/release/bundle/`
— **inúteis numa distro Arch-based**. O jeito de "instalar" aqui é
symlink direto do binário:

```sh
mkdir -p ~/.local/bin
ln -sf ~/quicktrad/src-tauri/target/release/quicktrad ~/.local/bin/quicktrad
```

(garanta que `~/.local/bin` está no `PATH`).

## 4. Dependências de sistema pra compilar (Arch/CachyOS)

```sh
sudo pacman -S --needed rust libappindicator-gtk3
```

`webkit2gtk-4.1` e `gtk3` normalmente já vêm instalados num desktop KDE/GNOME
padrão — só confira com `pacman -Q webkit2gtk-4.1 gtk3` antes de assumir que
falta.

## 5. Atalho global: cada compositor Wayland é um caso, não tem solução única no app

O app tenta registrar `Super+Shift+T` sozinho via
`tauri-plugin-global-shortcut` (funciona de verdade em Windows/macOS/X11).
Em **qualquer** Wayland (KDE, GNOME, Hyprland puro) isso falha silenciosamente
por design do protocolo — é esperado, não é bug, o código já trata isso com
um `eprintln!` de aviso em vez de crashar. A solução é sempre bindar
`quicktrad --toggle` no compositor/DE:

- **KDE Plasma 6**: não tem mais `khotkeys`/config simples de editar à mão.
  "Custom Shortcuts" agora é: um `.desktop` em
  `~/.local/share/applications/<id>.desktop` com
  `X-KDE-GlobalAccel-CommandShortcut=true`, mais uma linha
  `[services][<id>.desktop]` `_launch=<tecla>` em `~/.config/kglobalshortcutsrc`.
  Dá pra gerar os dois arquivos por script (não testamos automatizar isso
  ainda — se for fazer, teste e documente aqui o resultado).
- **GNOME**: idem, via GUI de Settings > Keyboard > Custom Shortcuts (ou
  `gsettings` no schema `org.gnome.settings-daemon.plugins.media-keys`).
- **Hyprland**: já documentado no README (`bindings.lua`).
- Tray icon (ícone "Mostrar/Ocultar") **não aparece em GNOME puro** — GNOME
  Shell não implementa `StatusNotifierItem` nativamente, precisa da extensão
  "AppIndicator and KStatusNotifierItem Support". Não é bug do app, não dá
  pra corrigir em código.

## 6. Ao testar mudanças de UI/janela, teste de verdade

Rodar `quicktrad` no terminal deste ambiente mostra a janela na tela real do
usuário (não é um sandbox isolado) — então dá pra validar visualmente pedindo
pro usuário olhar/printar a tela, mas **mate instâncias antigas antes**
(`pkill -9 -f quicktrad`; o `tauri-plugin-single-instance` faz uma instância
já rodando "engolir" a nova invocação via IPC, então testar sem matar a
antiga faz parecer que nada mudou mesmo depois de recompilar).
