import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { syncSystemTheme } from "./theme";

interface AppConfig {
  source_lang: string;
  target_lang: string;
  font_size: number;
}

let debounceTimer: number | undefined;
let fontSize = 14;
let swapInProgress = false;

const MIN_FONT_SIZE = 12;
const MAX_FONT_SIZE = 28;

const inputEl = () => document.querySelector<HTMLTextAreaElement>("#input")!;
const resultEl = () => document.querySelector<HTMLDivElement>("#result")!;
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
function renderFontSize(size: number) {
  document.documentElement.style.setProperty("--content-font-size", `${size}px`);
}

async function changeFontSize(delta: number) {
  const next = Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, fontSize + delta));
  if (next === fontSize) {
    return;
  }

  try {
    fontSize = await invoke<number>("set_font_size", { fontSize: next });
    renderFontSize(fontSize);
  } catch (err) {
    setResult(String(err), "error");
  }
}

function playPillPulse() {
  const text = langTextEl();
  text.getAnimations().forEach((animation) => animation.cancel());
  text.animate(
    [
      { opacity: 1, transform: "translateY(0) scale(1)" },
      { opacity: 0.5, transform: "translateY(-2px) scale(1.08)" },
      { opacity: 1, transform: "translateY(0) scale(1)" },
    ],
    { duration: 300, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
}

function playSwapOut() {
  const text = langTextEl();
  text.getAnimations().forEach((animation) => animation.cancel());
  text.animate(
    [
      { opacity: 1, transform: "translateY(0) rotateX(0deg)" },
      { opacity: 0, transform: "translateY(-5px) rotateX(70deg)" },
    ],
    { duration: 180, easing: "ease-in", fill: "forwards" },
  );

  document.querySelector<HTMLButtonElement>("#swap-btn")?.animate(
    [
      { transform: "rotate(0deg) scale(1)" },
      { transform: "rotate(180deg) scale(1.28)" },
      { transform: "rotate(360deg) scale(1)" },
    ],
    { duration: 480, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
}

function playSwapIn() {
  const text = langTextEl();
  text.getAnimations().forEach((animation) => animation.cancel());
  text.animate(
    [
      { opacity: 0, transform: "translateY(5px) rotateX(-70deg)" },
      { opacity: 1, transform: "translateY(0) rotateX(0deg)" },
    ],
    { duration: 240, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
  );
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
  if (swapInProgress) {
    return;
  }

  swapInProgress = true;
  playSwapOut();
  await new Promise((resolve) => window.setTimeout(resolve, 180));

  let cfg: AppConfig;
  try {
    cfg = await invoke<AppConfig>("swap_languages");
  } catch (err) {
    setResult(String(err), "error");
    swapInProgress = false;
    return;
  }

  renderPill(cfg);
  playSwapIn();

  if (resultEl().className === "text") {
    inputEl().value = resultEl().textContent ?? "";
    setResult("Tradução", "placeholder");
    await doTranslate();
  }

  inputEl().focus();
  swapInProgress = false;
}

window.addEventListener("DOMContentLoaded", async () => {
  void syncSystemTheme();
  inputEl().focus();
  inputEl().addEventListener("input", scheduleTranslate);

  document.querySelector("#swap-btn")?.addEventListener("click", () => {
    void doSwap();
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      void invoke("hide_window");
    } else if (
      e.ctrlKey &&
      e.altKey &&
      e.shiftKey &&
      (e.key === "+" || e.code === "Equal" || e.code === "NumpadAdd")
    ) {
      e.preventDefault();
      void changeFontSize(1);
    } else if (
      e.ctrlKey &&
      e.altKey &&
      e.shiftKey &&
      (e.key === "-" || e.code === "Minus" || e.code === "NumpadSubtract")
    ) {
      e.preventDefault();
      void changeFontSize(-1);
    } else if (e.key === "Tab") {
      e.preventDefault();
      void doSwap();
    }
  });

  window.addEventListener("focus", () => inputEl().focus());

  const cfg = await invoke<AppConfig>("get_config");
  fontSize = Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, cfg.font_size));
  renderFontSize(fontSize);
  renderPill(cfg);

  // Disparado pelo backend quando um novo par de idiomas chega via flag de
  // linha de comando (ex: outro bind do compositor invocou com --en --pt).
  await listen("config-updated", async () => {
    renderPill(await invoke<AppConfig>("get_config"));
    playPillPulse();
    if (inputEl().value.trim()) {
      void doTranslate();
    }
  });
});
