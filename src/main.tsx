import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MigrationBootBadge } from "./MigrationBootBadge";

const mountNodeId = "react-migration-root";
const existingMountNode = document.getElementById(mountNodeId);
const mountNode = existingMountNode ?? document.body.appendChild(document.createElement("div"));

if (!existingMountNode) {
  mountNode.id = mountNodeId;
}

createRoot(mountNode).render(
  <StrictMode>
    <MigrationBootBadge />
  </StrictMode>
);
