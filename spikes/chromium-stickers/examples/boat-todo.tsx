export const editableProps = {
  complete: { type: "boolean", default: false },
} as const;

export default function BoatTodo({ state }: { state: { complete: boolean } }) {
  const { complete } = state;
  return (
    <div style={{ position: "relative", width: 260, height: 140, transform: "rotate(3deg)", filter: "drop-shadow(0 8px 9px #000a)" }}>
      <svg viewBox="0 0 260 140" width="260" height="140" aria-hidden="true">
        <path d="M8 8H236L252 24V132H8Z" fill="#f5dd78" stroke="#856f2d" strokeWidth="3" strokeLinejoin="round"/>
        <path d="M236 8V24H252" fill="#ddc35f" stroke="#856f2d" strokeWidth="2" strokeLinejoin="round"/>
        <circle cx="130" cy="17" r="6" fill="#d65848" stroke="#873d36" strokeWidth="2"/>
        <path d="M23 35H237" stroke="#dfc761" strokeWidth="2"/>
        <rect x="27" y="55" width="27" height="27" rx="5" fill="#fff6cd" stroke="#78652b" strokeWidth="3"/>
        {complete ? <path d="M32 68l7 7 12-15" fill="none" stroke="#4c7749" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round"/> : null}
        <path d="M66 59H232M66 75H213M27 106H235" stroke="#d2bb59" strokeWidth="2" strokeLinecap="round"/>
        <path d="M22 126H236" stroke="#dfc761" strokeWidth="2"/>
      </svg>
      <div style={{ position: "absolute", left: 66, top: 52, width: 166, height: 42, display: "flex", alignItems: "center", color: "#30291a", font: "bold 14px 'DejaVu Sans Mono', monospace", lineHeight: 1.1, textDecoration: complete ? "line-through" : "none" }}>
        Pack up boat for season
      </div>
      <div style={{ position: "absolute", left: 28, top: 103, color: "#76632b", font: "bold 8px 'DejaVu Sans Mono', monospace", letterSpacing: 1 }}>
        {complete ? "DONE" : "TO DO"}
      </div>
    </div>
  );
}
