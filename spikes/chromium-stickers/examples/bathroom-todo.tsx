export const editableProps = {
  complete: { type: "boolean", default: false },
} as const;

type Props = { state: { complete: boolean } };

export default function BathroomTodo({ state }: Props) {
  const { complete } = state;
  return (
    <div style={{ position: "relative", width: 300, height: 160, transform: "rotate(-2deg)", filter: "drop-shadow(0 9px 10px #000a)" }}>
      <svg viewBox="0 0 300 160" width="300" height="160" aria-hidden="true">
        <path d="M8 8H276L292 24V152H8Z" fill="#f5dd78" stroke="#856f2d" strokeWidth="3" strokeLinejoin="round"/>
        <path d="M276 8V24H292" fill="#ddc35f" stroke="#856f2d" strokeWidth="2" strokeLinejoin="round"/>
        <circle cx="150" cy="18" r="7" fill="#d65848" stroke="#873d36" strokeWidth="2"/>
        <path d="M24 37H272" stroke="#dfc761" strokeWidth="2"/>
        <rect x="32" y="62" width="31" height="31" rx="5" fill="#fff6cd" stroke="#78652b" strokeWidth="3"/>
        {complete ? <path d="M38 77l8 8 14-18" fill="none" stroke="#4c7749" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round"/> : null}
        <path d="M76 67H260M76 83H228M32 116H260" stroke="#d2bb59" strokeWidth="2" strokeLinecap="round"/>
        <path d="M24 145H272" stroke="#dfc761" strokeWidth="2"/>
      </svg>
      <div style={{ position: "absolute", left: 76, top: 60, width: 186, height: 41, display: "flex", alignItems: "center", color: "#30291a", font: "bold 16px 'DejaVu Sans Mono', monospace", lineHeight: 1.1, textDecoration: complete ? "line-through" : "none" }}>
        Finish the bathroom
      </div>
      <div style={{ position: "absolute", left: 34, top: 112, color: "#76632b", font: "bold 9px 'DejaVu Sans Mono', monospace", letterSpacing: 1 }}>
        {complete ? "DONE" : "TO DO"}
      </div>
    </div>
  );
}
