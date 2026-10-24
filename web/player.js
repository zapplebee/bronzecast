const video = document.querySelector("#video");
const status = document.querySelector("#status");
const streamUrl = new URL("/stream/live.m3u8", location.origin).href;
const stage = document.querySelector(".stage-wrap");
async function toggleFullscreen() {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else if (stage.requestFullscreen) await stage.requestFullscreen();
    else if (video.webkitEnterFullscreen) video.webkitEnterFullscreen();
    else status.textContent = "FULLSCREEN IS NOT SUPPORTED BY THIS BROWSER";
  } catch (error) {
    console.error(error);
    status.textContent = "COULD NOT ENTER FULLSCREEN";
  }
}
document
  .querySelector("#fullscreen")
  .addEventListener("click", toggleFullscreen);
document
  .querySelector("#exit-fullscreen")
  .addEventListener("click", toggleFullscreen);
video.addEventListener("dblclick", toggleFullscreen);
document.addEventListener("keydown", (event) => {
  if (
    event.key.toLowerCase() === "f" &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    !["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName)
  ) {
    event.preventDefault();
    toggleFullscreen();
  }
});
const muteButton = document.querySelector("#mute");
muteButton.addEventListener("click", () => {
  video.muted = !video.muted;
  muteButton.textContent = video.muted ? "UNMUTE" : "MUTE";
  muteButton.setAttribute("aria-pressed", String(video.muted));
});
async function refreshStream() {
  const label = document.querySelector("#source");
  try {
    const response = await fetch("/api/stream", { cache: "no-store" });
    if (!response.ok) throw new Error("Playout unavailable");
    const state = await response.json();
    label.textContent =
      state.source === "live"
        ? "UDP INPUT LIVE / VIDEO + AUDIO"
        : "STANDBY CARD / WAITING FOR GENERATOR";
  } catch {
    label.textContent = "PLAYOUT SERVICE UNREACHABLE";
  }
}
refreshStream();
setInterval(refreshStream, 2000);

if (window.Hls?.isSupported()) {
  const hls = new Hls({
    liveSyncDurationCount: 2,
    manifestLoadingMaxRetry: 100,
    manifestLoadingRetryDelay: 1000,
  });
  hls.loadSource(streamUrl);
  hls.attachMedia(video);
  hls.on(Hls.Events.MANIFEST_PARSED, () => {
    status.textContent = "STREAM LIVE  //  H.264 OVER HLS";
    video.play().catch(() => {});
  });
  hls.on(Hls.Events.ERROR, (_, data) => {
    if (data.fatal) status.textContent = "WAITING FOR RENDERER…";
  });
} else if (video.canPlayType("application/vnd.apple.mpegurl")) {
  video.src = streamUrl;
} else {
  status.textContent = "THIS BROWSER DOES NOT SUPPORT HLS";
}

video.addEventListener(
  "playing",
  () => (status.textContent = "STREAM LIVE  //  H.264 OVER HLS"),
);
video.addEventListener(
  "waiting",
  () => (status.textContent = "BUFFERING LIVE SIGNAL…"),
);
document.querySelector("#play").addEventListener("click", (event) => {
  if (video.paused) {
    video.play();
    event.currentTarget.innerHTML = "Ⅱ <span>PAUSE</span>";
  } else {
    video.pause();
    event.currentTarget.innerHTML = "▶ <span>PLAY</span>";
  }
});

function castAvailable() {
  return Boolean(window.cast?.framework);
}
function initializeCast(available) {
  if (!available || !castAvailable()) return;
  cast.framework.CastContext.getInstance().setOptions({
    receiverApplicationId: chrome.cast.media.DEFAULT_MEDIA_RECEIVER_APP_ID,
    autoJoinPolicy: chrome.cast.AutoJoinPolicy.ORIGIN_SCOPED,
  });
  document.querySelector("#cast").classList.add("ready");
}
window.__onGCastApiAvailable = initializeCast;
const sdkPoll = setInterval(() => {
  if (castAvailable()) {
    initializeCast(true);
    clearInterval(sdkPoll);
  }
}, 250);

document.querySelector("#cast").addEventListener("click", async () => {
  if (!castAvailable()) {
    status.textContent = "OPEN IN CHROME TO USE GOOGLE CAST";
    return;
  }
  if (["localhost", "127.0.0.1"].includes(location.hostname)) {
    status.textContent = "OPEN THE LAN URL SO THE HUB CAN REACH THE STREAM";
    return;
  }
  try {
    const context = cast.framework.CastContext.getInstance();
    if (!context.getCurrentSession()) await context.requestSession();
    const session = context.getCurrentSession();
    if (!session) throw new Error("No Cast session");
    const media = new chrome.cast.media.MediaInfo(
      streamUrl,
      "application/x-mpegURL",
    );
    media.streamType = chrome.cast.media.StreamType.LIVE;
    media.metadata = new chrome.cast.media.GenericMediaMetadata();
    media.metadata.title = "BRONZECAST";
    media.metadata.subtitle = "Live character dashboard";
    const request = new chrome.cast.media.LoadRequest(media);
    request.autoplay = true;
    await session.loadMedia(request);
    status.textContent = "LIVE STREAM CAST TO THE HUB";
  } catch (error) {
    console.error(error);
    status.textContent = "CAST SESSION NOT STARTED";
  }
});

// Options come from the same registry as the renderer; adding a manifest entry
// automatically makes it available here without frontend model-ID branches.
const characterSelect = document.querySelector("#character");
let switching = false;
const writeKeyStorage = "bronzecast.writeKey";
function storedWriteKey() {
  try {
    return sessionStorage.getItem(writeKeyStorage) || "";
  } catch {
    return "";
  }
}
function rememberWriteKey(key) {
  try {
    sessionStorage.setItem(writeKeyStorage, key);
  } catch {
    /* storage can be blocked; the retry still carries the key */
  }
}
async function writeFetch(url, options) {
  const send = (key) =>
    fetch(url, {
      ...options,
      headers: {
        ...(options.headers || {}),
        ...(key ? { Authorization: `Bearer ${key}` } : {}),
      },
    });
  let response = await send(storedWriteKey());
  if (response.status !== 401) return response;
  const key = window.prompt(
    "This dashboard requires its write key to make changes:",
  );
  if (!key) return response;
  rememberWriteKey(key.trim());
  return send(key.trim());
}
async function refreshCharacters() {
  if (switching) return;
  try {
    const response = await fetch("/api/characters", { cache: "no-store" });
    if (!response.ok)
      throw new Error(`Character list: HTTP ${response.status}`);
    const state = await response.json();
    characterSelect.replaceChildren(
      ...state.characters.map((character) => {
        const option = document.createElement("option");
        option.value = character.id;
        option.textContent = character.name;
        return option;
      }),
    );
    characterSelect.value = state.requested_id;
    characterSelect.disabled = false;
  } catch (error) {
    characterSelect.disabled = true;
  }
}
characterSelect.addEventListener("change", async () => {
  switching = true;
  characterSelect.disabled = true;
  try {
    const response = await writeFetch("/api/character", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: characterSelect.value }),
    });
    if (!response.ok) throw new Error(await response.text());
    status.textContent = "CHARACTER REQUESTED — ALLOW FOR VIDEO DELAY";
  } catch (error) {
    console.error(error);
    status.textContent = "CHARACTER SWITCH FAILED";
  } finally {
    switching = false;
    await refreshCharacters();
  }
});
refreshCharacters();
setInterval(refreshCharacters, 5000);
