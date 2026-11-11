import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { DesktopApplication } from "@/desktop-application";
import "@/desktop-theme.css";

const container = document.getElementById("root");
if (!container) throw new Error("ARC desktop root is missing");
createRoot(container).render(
  <StrictMode>
    <DesktopApplication />
  </StrictMode>,
);
