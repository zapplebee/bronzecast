export default function GhostHead() {
  return (
    <svg viewBox="0 0 160 140" width="160" height="140" style={{ filter: "drop-shadow(0 6px 9px #0009)" }} aria-hidden="true">
      <path d="M24 125V71C24 37 47 14 80 14s56 23 56 57v54l-19-12-18 15-19-14-19 14-18-15z" fill="#edf3ed" stroke="#7eb5bc" strokeWidth="5" strokeLinejoin="round"/>
      <path d="M37 47c11-15 26-22 43-22M123 47c-11-15-26-22-43-22" fill="none" stroke="#fff" strokeWidth="5" strokeLinecap="round" opacity="0.8"/>
      <ellipse cx="59" cy="73" rx="9" ry="13" fill="#24343a"/><ellipse cx="101" cy="73" rx="9" ry="13" fill="#24343a"/>
      <circle cx="62" cy="70" r="3" fill="#fff"/><circle cx="104" cy="70" r="3" fill="#fff"/>
      <ellipse cx="80" cy="101" rx="11" ry="14" fill="#344248"/>
      <path d="M45 94q8 6 16 0M99 94q8 6 16 0" fill="none" stroke="#b4d4d4" strokeWidth="3" strokeLinecap="round"/>
    </svg>
  );
}
