import { useAnimationFrame } from "@bronzecast/sticker-runtime";

export const editableProps = {
  countdownTil: { type: "number", default: 0 },
} as const;

type Props = {
  state: { countdownTil: number };
};

export default function TimberwolvesHomeOpener({ state }: Props) {
  const now = useAnimationFrame();
  const remaining = Math.max(0, state.countdownTil - now);
  const days = Math.floor(remaining / 86_400_000);
  const hours = Math.floor((remaining % 86_400_000) / 3_600_000);
  const minutes = Math.floor((remaining % 3_600_000) / 60_000);
  const headline = remaining === 0
    ? "TIPOFF"
    : days > 0
      ? `${days} ${days === 1 ? "DAY" : "DAYS"}`
      : `${hours} HRS ${minutes} MIN`;

  return (
    <div style={{ position: "relative", width: 300, height: 160, filter: "drop-shadow(0 8px 10px #0009)" }}>
      <svg viewBox="0 0 300 160" width="300" height="160" aria-hidden="true">
        <rect x="7" y="7" width="286" height="146" rx="27" fill="#071c2c" stroke="#53c9ef" strokeWidth="4"/>
        <path d="M10 79H290M150 9V151M76 10Q137 79 76 150M224 10Q163 79 224 150" fill="none" stroke="#315873" strokeWidth="3"/>
        <path d="M23 23H63M237 137H277" stroke="#8bdcf2" strokeWidth="5" strokeLinecap="round"/>
        <circle cx="21" cy="137" r="5" fill="#8cc63e"/><circle cx="279" cy="23" r="5" fill="#8cc63e"/>
        <rect x="22" y="28" width="256" height="104" rx="17" fill="#e8f4f8" stroke="#53c9ef" strokeWidth="3"/>
      </svg>
      <div style={{ position: "absolute", inset: "34px 27px 32px", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", color: "#102a3e", fontFamily: "'DejaVu Sans Mono', monospace", textAlign: "center" }}>
        <div style={{ fontSize: 19, fontWeight: 800, letterSpacing: 0.5 }}>{headline}</div>
        <div style={{ marginTop: 6, fontSize: 8, fontWeight: 800, letterSpacing: 0.5 }}>TIMBERWOLVES HOME OPENER</div>
        <div style={{ marginTop: 4, fontSize: 8, fontWeight: 700 }}>VS TORONTO · 6 PM CT</div>
      </div>
    </div>
  );
}
