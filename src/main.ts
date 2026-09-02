import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface AppConfig {
  source_lang: string;
  target_lang: string;
}

let debounceTimer: number | undefined;

const inputEl = () => document.querySelector<HTMLTextAreaElement>("#input")!;
const resultEl = () => document.querySelector<HTMLDivElement>("#result")!;
const langPillEl = () => document.querySelector<HTMLDivElement>("#lang-pill")!;
const langTextEl = () => document.querySelector<HTMLSpanElement>("#lang-text")!;

function setResult(text: string, kind: "placeholder" | "text" | "error") {
  const el = resultEl();
  el.textContent = text;
  el.className = kind;
}

function renderPill(cfg: AppConfig) {
  langTextEl().textContent = `${cfg.source_lang.toUpperCase()} → ${cfg.target_lang.toUpperCase()}`;
}

// Reinicia a animação removendo e recolocando a classe num frame novo —
// senão dois swaps seguidos não re-disparam a animação CSS.
function flashPill() {
  const pill = langPillEl();
  pill.classList.remove("flash");
  void pill.offsetWidth;
  pill.classList.add("flash");
}

async function doTranslate() {
  const text = inputEl().value;
  if (!text.trim()) {
    setResult("Tradução", "placeholder");
    return;
  }
  try {
    const translated = await invoke<string>("translate", { text });
    if (translated) {
      setResult(translated, "text");
    } else {
      setResult("Tradução", "placeholder");
    }
  } catch (err) {
    setResult(String(err), "error");
  }
}

function scheduleTranslate() {
  window.clearTimeout(debounceTimer);
  debounceTimer = window.setTimeout(doTranslate, 350);
}

// Inverte o par de idiomas atual (pt→en vira en→pt) e, se já havia uma
// tradução na tela, sobe ela pro campo de entrada — igual ao botão de swap
// do Google Translate, pra ir e voltar entre duas línguas sem reconfigurar.
async function doSwap() {
  let cfg: AppConfig;
  try {
    cfg = await invoke<AppConfig>("swap_languages");
  } catch (err) {
    setResult(String(err), "error");
    return;
  }

  renderPill(cfg);
  flashPill();

  if (resultEl().className === "text") {
    inputEl().value = resultEl().textContent ?? "";
    setResult("Tradução", "placeholder");
    await doTranslate();
  }

  inputEl().focus();
}

window.addEventListener("DOMContentLoaded", async () => {
  inputEl().focus();
  inputEl().addEventListener("input", scheduleTranslate);

  document.querySelector("#swap-btn")?.addEventListener("click", () => {
    void doSwap();
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      void invoke("hide_window");
    } else if (e.key === "Tab") {
      e.preventDefault();
      void doSwap();
    }
  });

  window.addEventListener("focus", () => inputEl().focus());

  renderPill(await invoke<AppConfig>("get_config"));

  // Disparado pelo backend quando um novo par de idiomas chega via flag de
  // linha de comando (ex: outro bind do compositor invocou com --en --pt).
  await listen("config-updated", async () => {
    renderPill(await invoke<AppConfig>("get_config"));
    flashPill();
    if (inputEl().value.trim()) {
      void doTranslate();
    }
  });
});
