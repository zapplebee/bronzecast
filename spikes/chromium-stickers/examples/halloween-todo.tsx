export const editableProps = {
  task: { type: "string", default: "" },
  complete: { type: "boolean", default: false },
} as const;

type Props = { state: { task: string; complete: boolean } };

export default function HalloweenTodo({ state }: Props) {
  const task = state.task || "Halloween task";

  return (
    <div style={{ position: "relative", width: 180, height: 105, filter: "drop-shadow(0 6px 8px #000a)" }}>
      <svg viewBox="0 0 180 105" width="180" height="105" aria-hidden="true">
        <path d="M89 25c-20-12-38-8-49-1-23-7-38 9-37 29 1 25 20 39 42 31 14 9 30 8 44 2 14 6 30 7 44-2 22 8 41-6 42-31 1-20-14-36-37-29-11-7-29-11-49 1z" fill="#e87525" stroke="#68361c" strokeWidth="4"/>
        <path d="M48 26c-11 14-11 38-2 56M71 20c-6 20-6 47 0 66M108 20c6 20 6 47 0 66M132 26c11 14 11 38 2 56" fill="none" stroke="#b7511d" strokeWidth="3" strokeLinecap="round"/>
        <path d="M84 25c-3-10 0-17 8-22l10 2c-7 5-8 11-5 20z" fill="#634126" stroke="#362b20" strokeWidth="2"/>
        <rect x="19" y="36" width="142" height="48" rx="11" fill="#fff0cf" stroke="#744524" strokeWidth="2.5"/>
        <rect x="25" y="50" width="17" height="17" rx="3" fill="#fff9e8" stroke="#57432a" strokeWidth="2"/>
        {state.complete ? <path d="M28 58l5 5 7-10" fill="none" stroke="#4c7749" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"/> : null}
      </svg>
      <div style={{ position: "absolute", left: 46, top: 42, width: 107, height: 36, display: "flex", alignItems: "center", color: "#30271b", font: "bold 8px 'DejaVu Sans Mono', monospace", lineHeight: 1.05, textDecoration: state.complete ? "line-through" : "none" }}>
        {task}
      </div>
    </div>
  );
}
