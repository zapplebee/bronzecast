const video = document.querySelector("#video");
const status = document.querySelector("#status");
const streamUrl = new URL("/stream/live.m3u8", location.origin).href;

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
    console.error(error);
  }
}
characterSelect.addEventListener("change", async () => {
  switching = true;
  characterSelect.disabled = true;
  try {
    const response = await fetch("/api/character", {
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
