import type { CSSProperties } from "react";

const badgeStyle: CSSProperties = {
  position: "fixed",
  right: "16px",
  bottom: "16px",
  zIndex: 9999,
  padding: "6px 10px",
  borderRadius: "999px",
  fontSize: "12px",
  fontWeight: 700,
  color: "#d9e3ff",
  background: "rgba(16, 22, 37, 0.85)",
  border: "1px solid rgba(79, 124, 255, 0.45)",
  backdropFilter: "blur(6px)"
};

export function MigrationBootBadge() {
  return <div style={badgeStyle}>Vite + React + TS Booted</div>;
}
