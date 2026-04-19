import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import ResultView from "./result/ResultView";
import SettingsView from "./settings/SettingsView";

const label = getCurrentWindow().label;
const Root = label === "result" ? ResultView : label === "settings" ? SettingsView : App;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
