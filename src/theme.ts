import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "@tauri-apps/api/window";

/**
 * Ponto de extensão para detalhes que só existem numa plataforma. Por exemplo,
 * Windows pode fornecer a cor de destaque do usuário futuramente, sem mudar a
 * regra comum de tema claro/escuro usada no Windows, macOS e Linux.
 */
export interface ThemeExtension {
  apply(theme: Theme): void;
}

function browserTheme(): Theme {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function applyTheme(theme: Theme, extension?: ThemeExtension) {
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
  extension?.apply(theme);
}

/**
 * Segue o tema do sistema no app instalado e também no preview do Vite.
 * O media query mantém o fallback web funcionando em qualquer plataforma;
 * Tauri acrescenta a atualização nativa quando Windows/macOS/Linux avisam a
 * mudança de tema do sistema.
 */
export async function syncSystemTheme(extension?: ThemeExtension) {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const applyBrowserTheme = () => applyTheme(browserTheme(), extension);

  applyBrowserTheme();
  media.addEventListener("change", applyBrowserTheme);

  try {
    const appWindow = getCurrentWindow();
    const theme = await appWindow.theme();
    if (theme) {
      applyTheme(theme, extension);
    }

    const unlisten = await appWindow.onThemeChanged(({ payload: theme }) => {
      applyTheme(theme, extension);
    });

    return () => {
      media.removeEventListener("change", applyBrowserTheme);
      unlisten();
    };
  } catch {
    // No navegador (npm run dev) não existe bridge Tauri; o media query basta.
    return () => media.removeEventListener("change", applyBrowserTheme);
  }
}
