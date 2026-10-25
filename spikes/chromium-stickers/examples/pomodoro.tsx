export const editableProps = {
  currentDate: { type: "number", default: 0 },
  cycleStartedAt: { type: "number", default: 0 },
} as const;

type Props = {
  state: {
    currentDate: number;
    cycleStartedAt: number;
  };
};

const MINUTE = 60_000;
const FOCUS_LENGTH = 25 * MINUTE;
const BREAK_LENGTH = 5 * MINUTE;
const CYCLE_LENGTH = FOCUS_LENGTH + BREAK_LENGTH;

export default function Pomodoro({ state }: Props) {
  const position = ((state.currentDate - state.cycleStartedAt) % CYCLE_LENGTH + CYCLE_LENGTH) % CYCLE_LENGTH;
  const focusing = position < FOCUS_LENGTH;
  const remaining = Math.max(0, Math.ceil(((focusing ? FOCUS_LENGTH : CYCLE_LENGTH) - position) / 1000));
  const time = `${Math.floor(remaining / 60).toString().padStart(2, "0")}:${(remaining % 60).toString().padStart(2, "0")}`;
  const label = focusing ? "FOCUS" : "BREAK";

  return (
    <div style={{ position: "relative", width: 220, height: 140, filter: "drop-shadow(0 7px 9px #000a)" }}>
      <svg viewBox="0 0 220 140" width="220" height="140" aria-hidden="true">
        <path d="M110 25C64 4 19 29 17 75c-2 38 27 57 60 50 14 12 52 12 66 0 33 7 62-12 60-50-2-46-47-71-93-50z" fill="#d94132" stroke="#722b25" strokeWidth="5"/>
        <path d="M109 29c-8-14-5-24 8-27l13 3c-10 6-11 13-7 22z" fill="#5c432c" stroke="#352b20" strokeWidth="3"/>
        <path d="M118 26c11-17 26-21 39-13-13 4-20 11-26 21zM107 25C94 10 80 9 69 18c14 1 23 6 31 16z" fill="#5a8a42" stroke="#304b30" strokeWidth="3" strokeLinejoin="round"/>
        <path d="M33 74c2-24 17-39 39-42M187 74c-2-24-17-39-39-42" fill="none" stroke="#f4755b" strokeWidth="4" strokeLinecap="round"/>
        <rect x="43" y="48" width="134" height="64" rx="15" fill="#fff0d0" stroke="#71382b" strokeWidth="3"/>
        <circle cx="110" cy="121" r="3" fill="#f7c45f"/>
      </svg>
      <div style={{ position: "absolute", inset: "52px 46px 31px", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", color: "#38261e", fontFamily: "'DejaVu Sans Mono', monospace", textAlign: "center" }}>
        <div style={{ fontSize: 22, lineHeight: 1, fontWeight: 900, letterSpacing: 1 }}>{time}</div>
        <div style={{ marginTop: 5, fontSize: 8, fontWeight: 800, letterSpacing: 1.3 }}>{label}</div>
      </div>
    </div>
  );
}
