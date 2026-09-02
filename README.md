# quicktrad

Tradutor rápido minimalista: um popup sem decoração que você invoca por
atalho, digita, vê a tradução e fecha com `Esc`. Feito em Tauri (Rust +
webview), roda em Windows, macOS e Linux (X11 e Wayland/Hyprland).

## Rodando em desenvolvimento

```sh
npm install
npm run tauri dev
```

## UI

Só duas áreas de texto separadas por uma linha — sem header, sem seletor,
sem rodapé: digite em cima, a tradução aparece embaixo, `Esc` fecha,
`Tab` inverte o par de idioma atual (ver abaixo). Idioma não é escolhido
por dropdown; é definido por config ou por flag de CLI (abaixo) — a única
UI de idioma é um badge pequeno centralizado na própria linha divisória
("PT → EN ⇄"), só pra você saber em que direção está traduzindo.

### `Tab`: inverter idioma em execução

Com a janela aberta, `Tab` (ou clique no `⇄` do badge) inverte
origem↔destino do par **atual** (não troca pra um idioma diferente — só
espelha o que já está configurado, ex: pt→en vira en→pt). Se já havia uma
tradução na tela, ela sobe pro campo de entrada e é retraduzida na hora —
igual ao botão de swap do Google Translate. Pensado pro caso de "quero
digitar em inglês e ver como fica em PT-BR, ou vice-versa" sem precisar
fechar e reabrir com outra flag — dá pra ir alternando e completando a
frase nas duas línguas aos poucos. O badge pisca (texto e seta) por um
instante a cada troca, pra deixar a inversão visualmente óbvia sem
precisar de nenhum elemento extra na UI. Não funciona com
`source_lang = "auto"` (não tem pra onde inverter, já que não sabemos qual
idioma foi detectado) — nesse caso mostra um erro pedindo pra definir a
origem explicitamente.

## Como o atalho funciona em cada plataforma

- **No Omarchy** (uso principal hoje): `Super+Shift+T` abre/fecha o **widget
  da barra** (`omarchy-plugin/`, ver seção própria abaixo), não a janela
  flutuante. O bind está em `~/.config/hypr/bindings.lua`:

  ```lua
  o.bind("SUPER + SHIFT + T", "Quicktrad", "omarchy-shell guts.quicktrad toggle")
  ```

  `omarchy-shell <id> toggle` só repassa uma chamada IPC pro plugin já
  rodando dentro do Quickshell — não sobe processo novo.

- **Windows / macOS / Linux X11** (sem barra Omarchy): a própria aplicação
  registra `Super+Shift+T` como atalho global (via
  `tauri-plugin-global-shortcut`) e abre a **janela flutuante**. Funciona
  sem configurar nada a mais.
- **Outro Wayland/DE sem Quickshell** (GNOME, KDE): um app não pode
  capturar tecla globalmente sozinho — bind `quicktrad --toggle` no seu
  compositor/DE pra abrir a janela flutuante, igual ao Omarchy fazia antes
  do widget de barra existir.

A janela flutuante continua existindo e funcionando no Omarchy também —
`quicktrad --toggle` roda ela direto, sem depender da barra — só não tem
mais bind global padrão apontando pra ela lá, já que o uso principal agora
é o widget. Tray icon (`Mostrar/Ocultar` / `Sair`) disponível em qualquer
plataforma que rode a janela flutuante.

## Atalhos de teclado (resumo)

| Tecla | Onde | Ação |
|---|---|---|
| `Super+Shift+T` | Omarchy (compositor) | Abre/fecha o widget da barra |
| `Tab` | Dentro do widget da barra ou da janela flutuante, com o campo de texto focado | Inverte o par de idioma atual e retraduz |
| Clique no `⇄` | Widget da barra | Mesma ação do `Tab`, via mouse |
| `Esc` | Widget da barra ou janela flutuante | Fecha |

## Idioma: config ou flags de CLI

Arquivo em `~/.config/quicktrad/config.toml` (Linux/macOS) ou
`%APPDATA%\quicktrad\config.toml` (Windows), criado automaticamente na
primeira execução:

```toml
provider = "mymemory"      # mymemory | libretranslate | google | deepl
source_lang = "pt"
target_lang = "en"
libretranslate_url = "https://libretranslate.com/translate"

[api_keys]
deepl = "sua-chave-aqui"
libretranslate = "sua-chave-aqui"   # se a instância exigir
```

Ou passe o par de idiomas na hora de invocar, por flag — pensado pra ter um
bind por par (ex: um bind pt→en, outro en→pt, outro pt→ja):

```sh
quicktrad --pt --en   # origem pt, destino en
quicktrad --en        # só troca o destino, mantém a origem salva
```

Flags aceitas: `--auto --en --pt --es --fr --de --du --it --ja --jp --zh --ru`
(`--du` é alias de `--de`/alemão, `--jp` é alias de `--ja`/japonês). Se a
janela já estiver aberta, trocar o par **não fecha** — ela permanece e
re-traduz o texto atual na hora. Sem nenhuma flag de idioma, a invocação só
alterna mostrar/esconder (comportamento do `--toggle`).

Exemplo de múltiplos binds no Hyprland:

```lua
o.bind("SUPER + SHIFT + T", "Quicktrad PT→EN", { launch = "quicktrad --pt --en" })
o.bind("SUPER + SHIFT + E", "Quicktrad EN→PT", { launch = "quicktrad --en --pt" })
```

### Provedores de tradução

- **`mymemory`** (default): funciona sem cadastro nem API key. É uma
  translation memory, não um motor de MT puro — boa para frases curtas do
  dia a dia, mas pode errar em textos incomuns. Não faz auto-detecção de
  idioma (é preciso definir `source_lang` explicitamente, nunca `"auto"`).
- **`libretranslate`**: melhor se você hospedar sua própria instância
  (`libretranslate_url`) — a instância pública `libretranslate.com` passou a
  exigir API key paga. Suporta auto-detecção (`source_lang = "auto"`).
- **`google`**: endpoint público não-oficial do Google Translate, sem key,
  mas sujeito a bloqueio/rate-limit dependendo da rede.
- **`deepl`**: melhor qualidade, requer `api_keys.deepl` (tem plano free).

Adicionar um novo provedor é só implementar o trait `Provider` em
`src-tauri/src/translation.rs` e registrar no `build_provider`.

## Modo headless (CLI, sem abrir janela)

Além do modo GUI, o binário aceita três flags que rodam sem Tauri/GTK e
saem na hora — pensadas para integrações externas (é o que o plugin da
barra do Omarchy usa, ver abaixo):

```sh
quicktrad --query "bom dia"   # traduz e imprime no stdout, sai
quicktrad --swap              # inverte o par atual, imprime "origem destino", sai
quicktrad --status            # imprime o par atual "origem destino", sai
```

Todos leem/gravam o mesmo `config.toml` da janela flutuante — é a mesma
fonte de verdade, então usar um não desincroniza o outro.

## Integração nativa com a barra do Omarchy

Em `omarchy-plugin/` neste repositório: um widget de barra (Quickshell/QML)
no mesmo estilo dos cards nativos do Omarchy (wifi, bateria, mídia) — ícone
na barra que abre um card ancorado embaixo dele, com campo de texto e
tradução ali mesmo, sem abrir a janela flutuante. Ele não reimplementa
tradução em QML: só chama `quicktrad --query/--swap/--status` como
subprocesso, então compartilha config e providers com o app principal.
Dentro do card: `Tab` (com o campo focado) ou clique no `⇄` invertem o par
de idioma, igual à janela flutuante — ver [Atalhos de teclado](#atalhos-de-teclado-resumo).

Instalar (Linux com Omarchy):

```sh
mkdir -p ~/.config/omarchy/plugins/guts.quicktrad
cp omarchy-plugin/* ~/.config/omarchy/plugins/guts.quicktrad/
omarchy plugin enable guts.quicktrad
omarchy bar put guts.quicktrad --section right   # ou --section center, etc
omarchy restart shell
```

Depois, pra ter o `Super+Shift+T` abrindo o widget (em vez de só clicar o
ícone na barra), adicione em `~/.config/hypr/bindings.lua`:

```lua
o.bind("SUPER + SHIFT + T", "Quicktrad", "omarchy-shell guts.quicktrad toggle")
```

e rode `hyprctl reload`.

(Troque `guts` no id/pasta/bind pelo seu usuário/namespace se preferir —
não precisa ser literal.) O binário `quicktrad` precisa estar no `PATH`
(depois de `npm run tauri build`, aponte um symlink em
`~/.local/bin/quicktrad` pro binário gerado, ou instale o pacote/bundle).

**Nota pra quem for editar o QML**: mudanças em `Panel.qml` só recarregam
de verdade num widget de barra (`bar-widget`) depois de `omarchy restart
shell` — o hot-reload automático do Quickshell não recria a instância
persistente do widget, só re-executa o arquivo pra instâncias novas
(painéis avulsos tipo `disk-speedtest`). Editar e só fechar/abrir o card
não é suficiente.

Essa parte é **exclusiva do Omarchy/Quickshell** — nos outros ambientes
(GNOME, KDE, X11, Windows) o app continua funcionando normalmente como
popup flutuante via atalho global ou `--toggle`, só sem esse widget de
barra específico (não há um equivalente nativo de "plugin de barra" fora
do Quickshell para replicar essa parte).

## Build para distribuição

```sh
npm run tauri build
```

Gera instalador nativo por plataforma (NSIS/MSI no Windows, AppImage/deb no
Linux, dmg no macOS) — rode o comando no próprio SO alvo.

## Nota sobre este repositório de desenvolvimento

Se você testar `npm run tauri dev` dentro de um shell/agente com sandbox
restrito (namespaces limitados), pode ser necessário
`WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 npm run tauri dev` para o
webview conseguir inicializar. **Isso não é necessário na sua sessão de
desktop normal** — é só uma particularidade de rodar dentro de um ambiente
de automação sandboxed.

## Licença

[MIT](LICENSE).
