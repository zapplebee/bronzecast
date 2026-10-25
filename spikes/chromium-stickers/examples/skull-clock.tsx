import { useAnimationFrame } from "@bronzecast/sticker-runtime";

export const editableProps = {
  timeZone: { type: "string", default: "America/Chicago" },
} as const;

type Props = { state: { timeZone: string } };

export default function SkullClock({ state }: Props) {
  const date = new Date(useAnimationFrame());
  const time = new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    timeZone: state.timeZone,
  }).format(date);

  return (
    <div style={{ position: "relative", width: 300, height: 160, filter: "drop-shadow(0 7px 10px #000a)" }}>
      <svg viewBox="0 0 300 160" width="300" height="160" aria-hidden="true">
        <path d="M150 7C99 7 67 38 67 79c0 24 10 39 27 51v16h112v-16c17-12 27-27 27-51 0-41-32-72-83-72z" fill="#eee5ce" stroke="#574938" strokeWidth="4"/>
        <path d="M79 78c6-25 28-39 53-40M221 78c-6-25-28-39-53-40" fill="none" stroke="#c6b998" strokeWidth="4" strokeLinecap="round"/>
        <rect x="100" y="26" width="100" height="36" rx="6" fill="#263139" stroke="#574938" strokeWidth="3"/>
        <rect x="106" y="32" width="88" height="24" rx="3" fill="#132328" stroke="#64777a" strokeWidth="1"/>
        <path d="M90 91c0-14 13-24 27-19 9 3 14 12 13 22-1 14-10 25-22 24-11-1-18-13-18-27zM210 91c0-14-13-24-27-19-9 3-14 12-13 22 1 14 10 25 22 24 11-1 18-13 18-27z" fill="#263139"/>
        <circle cx="108" cy="91" r="4" fill="#fff8db"/><circle cx="192" cy="91" r="4" fill="#fff8db"/>
        <path d="M150 91l-11 22h22z" fill="#a77756" stroke="#574938" strokeWidth="2" strokeLinejoin="round"/>
        <path d="M106 122c12-8 27-8 44-2 17-6 32-6 44 2v19H106z" fill="#fbf6e8" stroke="#574938" strokeWidth="3" strokeLinejoin="round"/>
        <path d="M126 121v19M150 120v21M174 121v19" stroke="#9e9076" strokeWidth="2"/>
        <rect x="101" y="143" width="98" height="16" rx="8" fill="#263a43" stroke="#e2c875" strokeWidth="2"/>
      </svg>
      <div style={{ position: "absolute", left: 100, top: 26, width: 100, height: 36, display: "grid", placeItems: "center", color: "#b9ef75", font: "bold 13px 'DejaVu Sans Mono', monospace", letterSpacing: 0.4, textShadow: "0 0 6px #8cc63e" }}>
        {time}
      </div>
      <div style={{ position: "absolute", left: 101, top: 143, width: 98, height: 16, display: "grid", placeItems: "center", color: "#fff2cc", font: "bold 7px 'DejaVu Sans Mono', monospace", letterSpacing: 1 }}>
        LOCAL TIME
      </div>
    </div>
  );
}
