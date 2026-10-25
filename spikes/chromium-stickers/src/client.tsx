import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { MogStage } from "./MogStage";

type Sticker = {
  id: number;
  tsx: string;
  sourceHash: string;
  editableProps: Record<string, { type: string; default: unknown }>;
  state: Record<string, unknown>;
  x: number;
  y: number;
  angle: number;
  updatedAt: number;
};

function StickerComponent({ sticker }: { sticker: Sticker }) {
  const [Component, setComponent] = useState<React.ComponentType<{ state: Record<string, unknown> }> | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setComponent(null);
    setError(null);
    import(`/api/stickers/${sticker.id}/component.js?v=${sticker.sourceHash}`)
      .then(async (module) => {
        const response = await fetch(`/api/stickers/${sticker.id}/schema`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ editableProps: module.editableProps ?? {} }),
        });
        if (!response.ok) throw new Error(`State schema registration failed: ${response.status} ${await response.text()}`);
        if (active) setComponent(() => module.default);
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
    };
  }, [sticker.id, sticker.sourceHash]);

  return (
    <div
      className="sticker"
      style={{
        left: sticker.x,
        top: sticker.y,
        transform: `rotate(${sticker.angle}deg)`,
      }}
      data-sticker-id={sticker.id}
    >
      {error ? <pre className="error">{error}</pre> : Component ? <Component state={sticker.state} /> : null}
    </div>
  );
}

function Scene() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [apiError, setApiError] = useState("");

  useEffect(() => {
    let active = true;
    const refresh = async () => {
      try {
        const response = await fetch("/api/stickers", { cache: "no-store" });
        if (!response.ok) throw new Error(`Sticker API returned ${response.status}`);
        const next = (await response.json()) as Sticker[];
        if (active) {
          setStickers(next);
          setApiError("");
        }
      } catch (error) {
        if (active) setApiError(String(error));
      }
    };
    void refresh();
    const timer = window.setInterval(refresh, 200);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <main className="board" aria-label="960 by 540 sticker scene">
      <MogStage />
      <div className="stickers">
        {stickers.map((sticker) => <StickerComponent key={sticker.id} sticker={sticker} />)}
      </div>
      {apiError ? <output className="api-error">{apiError}</output> : null}
    </main>
  );
}

const root = createRoot(document.getElementById("root")!);
root.render(<Scene />);
window.__SPIKE_READY__ = true;
