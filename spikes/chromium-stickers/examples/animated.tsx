import React from "react";

type Props = { title: string };
export const editableProps = {
  title: { type: "string", default: "Live TSX" },
} as const;

export default function AnimatedSticker({ state }: { state: Props }) {
  const [hue, setHue] = React.useState(0);

  React.useEffect(() => {
    const timer = window.setInterval(() => setHue((value) => (value + 1) % 360), 80);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <div
      style={{
        width: 320,
        height: 160,
        borderRadius: 28,
        display: "grid",
        placeItems: "center",
        background: `hsl(${hue} 70% 48%)`,
        color: "white",
        font: "bold 32px sans-serif",
        boxShadow: "0 12px 50px #fff7",
      }}
    >
      {state.title}
    </div>
  );
}
