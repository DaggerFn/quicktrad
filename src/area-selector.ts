import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { syncSystemTheme } from "./theme";

const geometryEl = () => document.querySelector<HTMLOutputElement>("#geometry")!;
let saving = false;

async function updateGeometry() {
  const window = getCurrentWindow();
  const [position, size, scale] = await Promise.all([
    window.outerPosition(),
    window.innerSize(),
    window.scaleFactor(),
  ]);
  geometryEl().textContent = `x ${position.x} · y ${position.y} · ${Math.round(size.width / scale)} × ${Math.round(size.height / scale)} px`;
}

async function save() {
  if (saving) return;
  saving = true;
  try {
    await invoke("save_area_selection");
  } catch (error) {
    saving = false;
    geometryEl().textContent = String(error);
  }
}

async function cancel() {
  await invoke("cancel_area_selection");
}

window.addEventListener("DOMContentLoaded", async () => {
  await syncSystemTheme();
  document.querySelector("#save")?.addEventListener("click", () => void save());
  document.querySelector("#cancel")?.addEventListener("click", () => void cancel());
  document.addEventListener("keydown", (event) => {
    if (event.key === "Enter") void save();
    if (event.key === "Escape") void cancel();
  });

  const window = getCurrentWindow();
  await window.onMoved(() => void updateGeometry());
  await window.onResized(() => void updateGeometry());
  await updateGeometry();
});
