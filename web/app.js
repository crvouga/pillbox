import { Editor, weapons } from "./editor.js";
const $ = (id) => document.getElementById(id),
  clone = (v) => structuredClone(v),
  label = (s) => s.replaceAll("_", " ");
let configuring = null;
let user,
  templates,
  maps = [],
  modes = [],
  mode,
  modeContent = null,
  modeVersion = null,
  renderer,
  rendererPromise,
  room,
  ws,
  state,
  predicted,
  seq = 0,
  pending = [],
  snapshots = [],
  lastEvent = 0,
  lastPhase,
  matchId,
  finalResult = null,
  persisted = false,
  aim = { yaw: 0, pitch: 0 },
  fire = false,
  weapon = null,
  reload = false,
  speech = true,
  preview = false,
  networkRtt = 0,
  lastShot = 0;
const keys = new Set(),
  sent = new Map();
let nowTick = 0,
  accumulator = 0,
  lastFrame = performance.now(),
  scoreHeld = false;
function notify(message) {
  $("toast").textContent = message;
  $("toast").classList.remove("hidden");
  clearTimeout(notify.timer);
  notify.timer = setTimeout(() => $("toast").classList.add("hidden"), 6500);
}
async function api(path, method = "GET", body) {
  const response = await fetch("/api" + path, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  let value;
  try {
    value = await response.json();
  } catch {
    throw Error("The server returned an unexpected response");
  }
  if (!response.ok)
    throw Error(value.error || `Request failed (${response.status})`);
  return value;
}
function action(id, handler) {
  $(id).addEventListener("click", async (e) => {
    try {
      await handler(e);
    } catch (error) {
      notify(error.message);
    }
  });
}
const editor = new Editor((map) => {
  if (!room) renderer?.set_map(JSON.stringify(map));
}, notify);
function page(name) {
  document.body.classList.remove("preview");
  preview = false;
  document
    .querySelectorAll(".page")
    .forEach((p) => p.classList.toggle("hidden", p.id !== name));
  document
    .querySelectorAll(".nav")
    .forEach((b) => b.classList.toggle("active", b.dataset.page === name));
  if (name === "sandbox" && editor.map)
    renderer?.set_map(JSON.stringify(editor.map));
  if (name === "servers") {
    refreshServers().catch((e) => notify(e.message));
    renderer?.set_map(JSON.stringify(templates.map));
  }
  if (name === "sandbox") editor.draw();
}
document
  .querySelectorAll("[data-page]")
  .forEach((b) => (b.onclick = () => page(b.dataset.page)));
async function initRenderer() {
  if (rendererPromise) return rendererPromise;
  rendererPromise = (async () => {
    let backend = "webgl";
    if (navigator.gpu) {
      try {
        if (await navigator.gpu.requestAdapter()) backend = "webgpu";
      } catch {}
    }
    document.body.dataset.rendererPhase = "loading-" + backend;
    const mod = await import(`/pkg/${backend}/pillbox_client.js`);
    await mod.default();
    document.body.dataset.rendererPhase = "initializing";
    renderer = mod;
    mod.set_map(JSON.stringify(editor.map || templates.map));
    try {
      mod.run();
    } catch (e) {
      if (!String(e).includes("Using exceptions for control flow")) throw e;
    }
    document.body.dataset.rendererPhase = "running-" + backend;
    return mod;
  })();
  return rendererPromise;
}
async function refreshContent() {
  [maps, modes] = await Promise.all([
    api("/content/maps"),
    api("/content/modes"),
  ]);
  for (const [k, items] of [
    ["map", maps],
    ["mode", modes],
  ]) {
    const select = $(k + "-load"),
      old = select.value;
    select.replaceChildren(
      new Option(`Load saved ${k}…`, ""),
      ...items.map(
        (v) =>
          new Option(
            `${v.document.name} · v${v.version}${v.published ? " ↗" : " draft"}`,
            v.id,
          ),
      ),
    );
    select.value = old;
    const host = $("server-" + k);
    host.replaceChildren(
      ...items
        .filter((v) => v.published)
        .map((v) => new Option(`${v.document.name} · v${v.version}`, v.id)),
    );
  }
}
async function ensureStarters() {
  await refreshContent();
  if (!maps.some((v) => v.published)) {
    const v = await api("/content/maps", "POST", { document: templates.map });
    await api(`/content/maps/${v.id}/publish`, "POST");
  }
  if (!modes.some((v) => v.published)) {
    for (const mode of templates.modes) {
      const v = await api("/content/modes", "POST", { document: mode });
      await api(`/content/modes/${v.id}/publish`, "POST");
    }
  }
  await refreshContent();
}
async function signedIn() {
  $("identity").textContent = "● " + user.name;
  await ensureStarters();
  await refreshServers();
  const invite = new URL(location.href).searchParams.get("server");
  if (invite) await join(invite);
}
$("welcome-form").onsubmit = async (e) => {
  e.preventDefault();
  try {
    user = await api("/session", "POST", { name: $("player-name").value });
    $("welcome").close();
    await signedIn();
  } catch (e) {
    notify(e.message);
  }
};
async function refreshServers() {
  const list = await api("/servers");
  $("server-count").textContent = list.length;
  $("server-list").replaceChildren();
  if (!list.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent =
      "A universe of possibilities. No servers yet — create the first one.";
    $("server-list").append(empty);
  }
  for (const r of list) {
    const row = document.createElement("div");
    row.className = "server-row";
    const title = document.createElement("div"),
      strong = document.createElement("strong"),
      small = document.createElement("small");
    strong.textContent = r.config.name;
    small.textContent =
      maps.find((m) => m.id === r.config.map_version)?.document.name ||
      "Player-made arena";
    title.append(strong, small);
    const status = document.createElement("span");
    status.className = "tag";
    status.textContent = r.phase;
    const count = document.createElement("span");
    count.textContent = `${r.players} / ${r.config.max_players} pills`;
    const button = document.createElement("button");
    button.textContent = "Join →";
    button.disabled = r.players >= r.config.max_players;
    button.onclick = () => join(r.id).catch((e) => notify(e.message));
    const actions = document.createElement("div");
    actions.className = "actions";
    if (user && r.owner === user.id) {
      const settings = document.createElement("button");
      settings.textContent = "Configure";
      settings.onclick = async () => {
        try {
          await refreshContent();
          configuring = r.id;
          $("server-name").value = r.config.name;
          $("server-map").value = r.config.map_version;
          $("server-mode").value = r.config.mode_version;
          $("server-max").value = r.config.max_players;
          $("server-visibility").value = r.config.visibility;
          $("host-submit").textContent = "Save & join →";
          $("host-dialog").showModal();
        } catch (e) {
          notify(e.message);
        }
      };
      actions.append(settings);
    }
    actions.append(button);
    row.append(title, status, count, actions);
    $("server-list").append(row);
  }
}
action("refresh", refreshServers);
action("join-direct", () => {
  let id = $("join-code").value.trim();
  try {
    id = new URL(id).searchParams.get("server") || id;
  } catch {}
  return join(id);
});
action("host-open", async () => {
  configuring = null;
  $("host-submit").textContent = "Create & join →";
  await refreshContent();
  $("host-dialog").showModal();
});
action("host-cancel", () => $("host-dialog").close());
$("host-form").onsubmit = async (e) => {
  e.preventDefault();
  try {
    const r = await api(
      configuring ? "/servers/" + configuring : "/servers",
      configuring ? "PUT" : "POST",
      {
        name: $("server-name").value,
        max_players: +$("server-max").value,
        visibility: $("server-visibility").value,
        map_version: $("server-map").value,
        mode_version: $("server-mode").value,
      },
    );
    $("host-dialog").close();
    await join(r.id);
  } catch (e) {
    notify(e.message);
  }
};
async function saveMap(publish = false) {
  const v = await api("/content/maps", "POST", {
    content_id: editor.contentId,
    document: editor.map,
  });
  if (publish) {
    await api(`/content/maps/${v.id}/publish`, "POST");
    v.published = true;
  }
  editor.contentId = v.content_id;
  editor.version = v;
  editor.dirty = false;
  editor.changed(false);
  await refreshContent();
  notify(publish ? "Map published. It is ready to host." : "Map draft saved.");
  return v;
}
action("map-save", () => saveMap());
action("map-publish", () => saveMap(true));
action("map-new", () => {
  editor.load({
    ...clone(templates.map),
    name: "Untitled arena",
    objects: clone(
      templates.map.objects.filter(
        (o) => o.kind === "floor" || o.kind === "team_spawn",
      ),
    ),
  });
});
$("map-load").onchange = () => {
  const v = maps.find((v) => v.id === $("map-load").value);
  if (v)
    editor.load(v.document, v.owner_id === user.id ? v.content_id : null, v);
};
action("preview-toggle", () => {
  preview = !preview;
  document.body.classList.toggle("preview", preview);
});
action("map-test", async () => {
  const v = await saveMap(true);
  const selected = modes.find(
    (m) => m.published && m.document.objective === "none",
  );
  if (!selected) throw Error("Publish a mode first");
  const r = await api("/servers", "POST", {
    name: "Test · " + v.document.name,
    max_players: 8,
    visibility: "unlisted",
    map_version: v.id,
    mode_version: selected.id,
  });
  await join(r.id);
  notify("Test arena ready. Start the match, or copy its invite for a friend.");
});
function loadMode(value, contentId = null, version = null) {
  mode = clone(value);
  modeContent = contentId;
  modeVersion = version;
  for (const [id, key] of [
    ["mode-name", "name"],
    ["mode-score", "score_limit"],
    ["mode-time", "time_limit"],
    ["mode-respawn", "respawn_seconds"],
    ["mode-objective", "objective"],
  ])
    $(id).value = mode[key];
  $("mode-teams").checked = mode.teams;
  $("mode-friendly").checked = mode.friendly_fire;
  $("mode-rules").value = JSON.stringify(mode.rules, null, 2);
  $("mode-version").textContent = version
    ? `Version ${version.version} · ${version.published ? "published and immutable" : "draft"}`
    : "New mode";
}
async function saveMode(publish = false) {
  mode = {
    name: $("mode-name").value,
    score_limit: +$("mode-score").value,
    time_limit: +$("mode-time").value,
    respawn_seconds: +$("mode-respawn").value,
    objective: $("mode-objective").value,
    teams: $("mode-teams").checked,
    friendly_fire: $("mode-friendly").checked,
    rules: JSON.parse($("mode-rules").value),
  };
  const v = await api("/content/modes", "POST", {
    content_id: modeContent,
    document: mode,
  });
  if (publish) {
    await api(`/content/modes/${v.id}/publish`, "POST");
    v.published = true;
  }
  loadMode(mode, v.content_id, v);
  await refreshContent();
  notify(
    publish
      ? "Mode published. Make a server with your rules."
      : "Mode draft saved.",
  );
}
action("mode-save", () => saveMode());
action("mode-publish", () => saveMode(true));
$("mode-load").onchange = () => {
  const v = modes.find((v) => v.id === $("mode-load").value);
  if (v) loadMode(v.document, v.owner_id === user.id ? v.content_id : null, v);
};
async function join(id) {
  if (!user) {
    $("welcome").showModal();
    return;
  }
  if (room) leave();
  const data = await api(`/servers/${encodeURIComponent(id)}`);
  notify("Loading your arena…");
  await initRenderer();
  room = data;
  state = data.state;
  renderer.set_map(JSON.stringify(data.map));
  predicted = null;
  pending = [];
  snapshots = [];
  seq = 0;
  lastEvent = 0;
  lastPhase = null;
  persisted = false;
  matchId = data.server.match_id;
  finalResult = null;
  aim = { yaw: 0, pitch: 0 };
  keys.clear();
  fire = false;
  document.body.classList.add("playing");
  document.body.classList.remove("preview");
  $("match-ui").classList.remove("hidden");
  $("match-title").textContent = data.server.config.name;
  const owner = data.server.owner === user.id;
  $("match-start").classList.toggle("hidden", !owner);
  $("match-finish").classList.toggle("hidden", !owner);
  const url = new URL(location.href);
  url.searchParams.set("server", id);
  history.replaceState(null, "", url);
  ws = new WebSocket(
    `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/api/servers/${id}/ws`,
  );
  const socket = ws;
  socket.onmessage = (e) => {
    if (socket !== ws) return;
    const packet = JSON.parse(e.data);
    receive(packet);
  };
  socket.onclose = () => {
    if (socket === ws && room) {
      leave();
      notify(
        "Disconnected. Your match results and published content stay saved. Rejoin from the server browser.",
      );
    }
  };
  socket.onerror = () =>
    notify(
      "Unable to join. This browser may already be connected, or the server is full.",
    );
}
function leave() {
  const socket = ws;
  ws = null;
  room = null;
  socket?.close();
  document.exitPointerLock?.();
  document.body.classList.remove("playing");
  $("match-ui").classList.add("hidden");
  $("scoreboard").classList.add("hidden");
  predicted = null;
  state = null;
  finalResult = null;
  pending = [];
  sent.clear();
  keys.clear();
  fire = false;
  history.replaceState(null, "", location.pathname);
  renderer?.set_map(JSON.stringify(templates.map));
  page("servers");
}
action("leave", leave);
action("match-start", () => api(`/servers/${room.server.id}/start`, "POST"));
action("match-finish", () => api(`/servers/${room.server.id}/finish`, "POST"));
action("invite", async () => {
  await navigator.clipboard.writeText(location.href);
  notify("Invite copied. Open it in another browser to join as another pill.");
});
action("sound", () => {
  speech = !speech;
  $("sound").textContent = "Announcer: " + (speech ? "on" : "off");
  if (!speech) speechSynthesis.cancel();
});
function receive(packet) {
  const old = state;
  state = packet.state;
  if (matchId !== packet.match_id) finalResult = null;
  matchId = packet.match_id;
  persisted = packet.persisted;
  nowTick = state.tick;
  snapshots.push({ time: performance.now(), state: clone(state) });
  while (snapshots.length > 8) snapshots.shift();
  const local = state.players.find((p) => p.id === user.id);
  if (local) {
    if (sent.has(local.ack)) {
      networkRtt = performance.now() - sent.get(local.ack);
    }
    for (const k of sent.keys()) if (k <= local.ack) sent.delete(k);
    pending = pending.filter((i) => i.seq > local.ack);
    if (pending.length > 120) pending = [];
    const corrected = clone(local);
    corrected.yaw = aim.yaw;
    corrected.pitch = aim.pitch;
    predicted = JSON.parse(
      renderer.reconcile(
        JSON.stringify(corrected),
        JSON.stringify(state.phase === "playing" ? pending : []),
        state.tick,
      ),
    );
    const previous = old?.players.find((p) => p.id === user.id);
    if (
      previous &&
      previous.health + previous.shield > local.health + local.shield
    ) {
      $("damage-flash").classList.add("hit");
      setTimeout(() => $("damage-flash").classList.remove("hit"), 200);
    }
  }
  for (const event of state.events) {
    if (event.id <= lastEvent) continue;
    lastEvent = event.id;
    if (event.kind === "player_killed") {
      const entry = document.createElement("div");
      const name = (id) =>
        state.players.find((p) => p.id === id)?.name || "A pill";
      entry.textContent = `${name(event.actor)} eliminated ${name(event.target)} · ${label(event.weapon || "arena")}`;
      $("kill-feed").prepend(entry);
      setTimeout(() => entry.remove(), 6000);
    } else if (event.kind !== "shot" && event.kind !== "explosion") {
      announce(event.kind);
    }
  }
  if (state.phase !== lastPhase) {
    if (state.phase === "starting") {
      pending = [];
      lastEvent = state.events.at(-1)?.id || lastEvent;
    }
    if (state.phase === "finished") {
      document.exitPointerLock?.();
      fire = false;
    }
    lastPhase = state.phase;
  }
  if (
    state.phase === "finished" &&
    persisted &&
    finalResult?.matchId !== matchId
  ) {
    const id = matchId;
    finalResult = { matchId: id, participants: null };
    api(`/results/${id}`)
      .then((result) => {
        if (!room || matchId !== id) return;
        finalResult = { matchId: id, participants: result.participants };
        const mine = result.participants.find((p) => p.id === user.id);
        if (mine) {
          const scores = room.mode.teams
            ? result.state.team_scores
            : result.participants.map((p) => p.score);
          const score = room.mode.teams ? scores[mine.team] : mine.score;
          announce(score === Math.max(...scores) ? "victory" : "defeat");
        }
        updateHud();
      })
      .catch((error) => {
        if (room && matchId === id)
          notify("Could not load the final scoreboard: " + error.message);
      });
  }
  updateHud();
}
function announce(kind) {
  const text = label(kind);
  $("announcement").textContent = text.toUpperCase();
  clearTimeout(announce.timer);
  announce.timer = setTimeout(() => ($("announcement").textContent = ""), 2600);
  if (speech && "speechSynthesis" in window) {
    speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.rate = 1.05;
    utterance.volume = 0.55;
    speechSynthesis.speak(utterance);
  }
}
function updateHud() {
  if (!state || !room) return;
  const p = state.players.find((p) => p.id === user.id);
  if (p) {
    $("shield").value = p.shield;
    $("health").textContent = Math.ceil(p.health) + " HEALTH";
    $("weapon").textContent =
      label(p.weapon) + (p.reload_at ? " · RELOADING" : "");
    const wi = weapons.indexOf(p.weapon);
    $("ammo").textContent = `${p.magazines[wi]} / ${p.reserve[wi]}`;
  }
  $("clock").textContent =
    `${Math.floor(state.remaining / 60)}:${String(state.remaining % 60).padStart(2, "0")}`;
  $("team-score").textContent = room.mode.teams
    ? `CYAN ${state.team_scores[0]} : ${state.team_scores[1]} CORAL`
    : `${p?.score || 0} / ${room.mode.score_limit}`;
  $("net-status").textContent = `${Math.round(networkRtt)} ms ack · 60 Hz`;
  $("phase-banner").textContent =
    state.phase === "waiting"
      ? "Waiting for the owner to start the match"
      : state.phase === "starting"
        ? "Get ready…"
        : state.phase === "finished"
          ? "Match complete"
          : !p?.alive
            ? `Respawning in ${Math.max(1, Math.ceil((p.respawn_at - state.tick) / 60))}…`
            : document.pointerLockElement
              ? ""
              : "Click the arena to aim";
  $("match-start").disabled =
    !["waiting", "finished"].includes(state.phase) ||
    (state.phase === "finished" && !persisted);
  $("match-start").textContent =
    state.phase === "finished" ? "Play again" : "Start match";
  $("match-finish").disabled = !["starting", "playing"].includes(state.phase);
  $("scoreboard").classList.toggle(
    "hidden",
    !scoreHeld && state.phase !== "finished",
  );
  const table = document.createElement("table"),
    head = document.createElement("tr");
  for (const text of ["Pill", "Team", "Score", "Kills", "Deaths"]) {
    const th = document.createElement("th");
    th.textContent = text;
    head.append(th);
  }
  table.append(head);
  for (const q of [...(finalResult?.participants || state.players)].sort(
    (a, b) => b.score - a.score,
  )) {
    const tr = document.createElement("tr");
    for (const value of [
      q.name + (q.id === user.id ? " (you)" : ""),
      room.mode.teams ? (q.team ? "Coral" : "Cyan") : "—",
      q.score,
      q.kills,
      q.deaths,
    ]) {
      const td = document.createElement("td");
      td.textContent = value;
      tr.append(td);
    }
    table.append(tr);
  }
  $("score-rows").replaceChildren(table);
  $("result-status").replaceChildren();
  if (state.phase === "finished") {
    $("result-status").textContent = persisted
      ? "Match result saved. "
      : "Saving match result…";
    if (persisted) {
      const a = document.createElement("a");
      a.href = "/api/results/" + matchId;
      a.target = "_blank";
      a.rel = "noopener";
      a.textContent = "View durable result";
      $("result-status").append(a);
    }
  }
}
function localInput() {
  const active =
    document.pointerLockElement === $("game") && state?.phase === "playing";
  return {
    seq: ++seq,
    x: active ? Number(keys.has("KeyD")) - Number(keys.has("KeyA")) : 0,
    z: active ? Number(keys.has("KeyS")) - Number(keys.has("KeyW")) : 0,
    yaw: aim.yaw,
    pitch: aim.pitch,
    jump: active && keys.has("Space"),
    fire: active && fire,
    reload: active && reload,
    weapon: active ? weapon : null,
    view_tick: Math.max(0, nowTick - 6),
  };
}
window.addEventListener("keydown", (e) => {
  if (!room) return;
  if (["Space", "Tab", "KeyW", "KeyA", "KeyS", "KeyD"].includes(e.code))
    e.preventDefault();
  keys.add(e.code);
  if (e.code === "Tab") {
    scoreHeld = true;
    updateHud();
  }
  if (e.code === "KeyR") reload = true;
  const n = +e.key;
  if (n >= 1 && n <= 6) weapon = weapons[n - 1];
});
window.addEventListener("keyup", (e) => {
  keys.delete(e.code);
  if (e.code === "Tab") {
    scoreHeld = false;
    updateHud();
  }
  if (e.code === "KeyR") reload = false;
});
window.addEventListener("blur", () => {
  keys.clear();
  fire = false;
  reload = false;
});
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    keys.clear();
    fire = false;
    pending = [];
  }
});
document.addEventListener("pointerlockchange", () => {
  if (!document.pointerLockElement) {
    keys.clear();
    fire = false;
  }
  if (room) updateHud();
});
$("game").addEventListener("mousedown", (e) => {
  if (!room) return;
  if (document.pointerLockElement !== $("game")) {
    const lock = $("game").requestPointerLock?.();
    lock?.catch(() =>
      notify("Click the focused arena window to enable mouse aiming."),
    );
    return;
  }
  if (e.button === 0) fire = true;
});
window.addEventListener("mouseup", () => (fire = false));
window.addEventListener("mousemove", (e) => {
  if (document.pointerLockElement !== $("game")) return;
  aim.yaw = (aim.yaw + e.movementX * 0.0025) % (Math.PI * 2);
  aim.pitch = Math.max(-1.5, Math.min(1.5, aim.pitch - e.movementY * 0.0025));
});
$("game").oncontextmenu = (e) => e.preventDefault();
function animate(time) {
  const dt = Math.min((time - lastFrame) / 1000, 0.1);
  lastFrame = time;
  accumulator += dt;
  if (ws?.readyState === WebSocket.OPEN) {
    let steps = 0;
    while (accumulator >= 1 / 60 && steps++ < 6) {
      accumulator -= 1 / 60;
      const input = localInput();
      if (ws.bufferedAmount < 16000) {
        ws.send(JSON.stringify(input));
        sent.set(input.seq, time);
      }
      if (state?.phase === "playing" && predicted) {
        pending.push(input);
        if (pending.length > 120) pending.shift();
        predicted = JSON.parse(
          renderer.predict(JSON.stringify(input), nowTick + pending.length),
        );
      }
      if (input.fire && predicted?.alive && time - lastShot > 90) {
        lastShot = time;
        $("crosshair").classList.add("firing");
        setTimeout(() => $("crosshair").classList.remove("firing"), 60);
      }
      weapon = null;
      reload = false;
    }
  } else accumulator = 0;
  if (renderer) {
    if (room && state) {
      let rendered = state;
      const target = time - 100;
      let a = snapshots[0],
        b = snapshots.at(-1);
      for (let i = 1; i < snapshots.length; i++)
        if (snapshots[i].time >= target) {
          a = snapshots[i - 1];
          b = snapshots[i];
          break;
        }
      if (a && b) {
        const t = Math.max(
          0,
          Math.min(1, (target - a.time) / Math.max(1, b.time - a.time)),
        );
        rendered = clone(b.state);
        for (const p of rendered.players) {
          const old = a.state.players.find((q) => q.id === p.id);
          if (
            old &&
            old.alive &&
            p.alive &&
            Math.hypot(...p.position.map((v, i) => v - old.position[i])) < 5
          )
            p.position = p.position.map(
              (v, i) => old.position[i] + (v - old.position[i]) * t,
            );
        }
      }
      if (predicted) {
        predicted.yaw = aim.yaw;
        predicted.pitch = aim.pitch;
      }
      renderer.set_frame(
        JSON.stringify({
          state: rendered,
          local_id: user.id,
          predicted,
          camera: null,
          target: null,
        }),
      );
    } else {
      const orbit = preview ? time * 0.00006 : 0.7;
      renderer.set_frame(
        JSON.stringify({
          state: null,
          local_id: "",
          predicted: null,
          camera: [Math.sin(orbit) * 42, 32, Math.cos(orbit) * 42],
          target: [0, 0, 0],
        }),
      );
    }
  }
  requestAnimationFrame(animate);
}
(async () => {
  try {
    templates = await api("/templates");
    editor.load(templates.map);
    loadMode(templates.modes[0]);
    for (const m of templates.modes) {
      const b = document.createElement("button");
      b.textContent = m.name;
      b.onclick = () => loadMode(m);
      $("mode-templates").append(b);
    }
    initRenderer().catch((e) => {
      document.body.dataset.rendererPhase = "failed";
      notify("3D renderer could not start: " + String(e.message || e));
    });
    try {
      user = await api("/session");
      await signedIn();
    } catch (e) {
      if (!user) $("welcome").showModal();
      else throw e;
    }
    requestAnimationFrame(animate);
  } catch (e) {
    notify(e.message);
  }
})();
setInterval(() => {
  if (!room && !$("servers").classList.contains("hidden"))
    refreshServers().catch(() => {});
}, 10000);

setInterval(() => {
  if (document.hidden && ws?.readyState === WebSocket.OPEN) {
    const input = localInput();
    input.x = 0;
    input.z = 0;
    input.fire = false;
    input.jump = false;
    input.reload = false;
    input.weapon = null;
    ws.send(JSON.stringify(input));
  }
}, 1000);
