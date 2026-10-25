export const editableProps = {
  currentDate: { type: "number", default: 0 },
  countdownTil: { type: "number", default: 0 },
} as const;

type Props = {
  state: {
    currentDate: number;
    countdownTil: number;
  };
};

function calendarDay(timestamp: number) {
  const date = new Date(timestamp);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

export default function HalloweenCountdown({ state }: Props) {
  const days = Math.max(0, Math.round((calendarDay(state.countdownTil) - calendarDay(state.currentDate)) / 86_400_000));
  const headline = days === 0 ? "TODAY" : `${days} ${days === 1 ? "DAY" : "DAYS"}`;

  return (
    <div style={{ position: "relative", width: 300, height: 160, filter: "drop-shadow(0 8px 10px #0009)" }}>
      <svg viewBox="0 0 300 160" width="300" height="160" aria-hidden="true">
        <path d="M145 34C114 15 87 20 70 32 33 19 12 47 14 83c2 40 32 60 69 47 22 12 43 11 67 1 25 10 46 11 68-1 37 13 67-7 69-47 2-36-19-64-56-51-17-12-44-17-75 2z" fill="#eb7a24" stroke="#713719" strokeWidth="5"/>
        <path d="M75 31c-19 23-20 63-3 94M111 25c-11 30-11 75 0 104M188 25c11 30 11 75 0 104M225 31c19 23 20 63 3 94" fill="none" stroke="#b6501b" strokeWidth="4" strokeLinecap="round"/>
        <path d="M137 33c-4-14-1-24 9-30l15 3c-10 8-12 17-8 29z" fill="#694123" stroke="#392b1e" strokeWidth="3"/>
        <path d="M154 29c13-17 29-20 42-12-15 3-23 10-29 19z" fill="#54823c" stroke="#34572d" strokeWidth="2"/>
        <rect x="39" y="47" width="222" height="70" rx="17" fill="#fff1d1" stroke="#754423" strokeWidth="3"/>
        <path d="M139 127l8-5 8 5 8-5 8 5" fill="none" stroke="#66391f" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
      <div style={{ position: "absolute", inset: "47px 42px 43px", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", color: "#332116", fontFamily: "'DejaVu Sans Mono', monospace", textAlign: "center" }}>
        <div style={{ fontSize: 19, fontWeight: 800, letterSpacing: 1 }}>{headline}</div>
        <div style={{ marginTop: 7, fontSize: 9, fontWeight: 700, letterSpacing: 1.2 }}>UNTIL HALLOWEEN</div>
      </div>
    </div>
  );
}
