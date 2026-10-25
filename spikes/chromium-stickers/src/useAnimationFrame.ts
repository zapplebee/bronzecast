import { useSyncExternalStore } from "react";

type Listener = () => void;

const listeners = new Set<Listener>();
let currentTime = 0;
let request = 0;

function tick() {
  request = 0;
  currentTime = Date.now();
  for (const listener of listeners) listener();
  if (listeners.size > 0) request = requestAnimationFrame(tick);
}

function subscribe(listener: Listener) {
  listeners.add(listener);
  if (request === 0) request = requestAnimationFrame(tick);
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && request !== 0) {
      cancelAnimationFrame(request);
      request = 0;
    }
  };
}

function getSnapshot() {
  return currentTime;
}

/** Shared wall-clock time refreshed by one requestAnimationFrame loop per page. */
export function useAnimationFrame() {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
