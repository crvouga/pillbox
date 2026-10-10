import { test } from "node:test";
import assert from "node:assert/strict";
import WebSocket from "ws";
const base = process.env.TEST_URL || "http://127.0.0.1:8080";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function request(path, method = "GET", body, cookie) {
  const r = await fetch(base + path, {
    method,
    headers: {
      ...(body ? { "Content-Type": "application/json" } : {}),
      ...(cookie ? { Cookie: cookie } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  return {
    status: r.status,
    data: await r.json(),
    cookie: r.headers.get("set-cookie")?.split(";")[0],
  };
}
async function until(fn, message, ms = 12000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const v = await fn();
    if (v) return v;
    await sleep(30);
  }
  throw Error("Timed out: " + message);
}
function connect(id, cookie) {
  const ws = new WebSocket(
    base.replace(/^http/, "ws") + "/api/servers/" + id + "/ws",
    { headers: { Cookie: cookie } },
  );
  const client = { ws, packet: null, seq: 0 };
  ws.on("message", (data) => (client.packet = JSON.parse(data)));
  return new Promise((resolve, reject) => {
    ws.once("open", () => resolve(client));
    ws.once("error", reject);
  });
}
function send(c, overrides = {}) {
  c.ws.send(
    JSON.stringify({
      seq: ++c.seq,
      x: 0,
      z: 0,
      yaw: 0,
      pitch: 0,
      jump: false,
      fire: false,
      reload: false,
      weapon: null,
      view_tick: c.packet?.state.tick || 0,
      ...overrides,
    }),
  );
}
test(
  "creator → publish → two clients → combat → durable result → reuse",
  { timeout: 45000 },
  async (t) => {
    assert.equal((await request("/health")).status, 200);
    const a = await request("/api/session", "POST", {
        name: "Acceptance Alice",
      }),
      b = await request("/api/session", "POST", { name: "Acceptance Bob" });
    assert.equal(a.status, 200);
    assert.equal(b.status, 200);
    assert.ok(a.cookie);
    const templates = (await request("/api/templates")).data;
    const map = {
      ...templates.map,
      name: "Acceptance arena",
      objects: [
        ...templates.map.objects.filter(
          (o) => o.kind === "floor" && o.position[1] < 0,
        ),
        ...templates.map.objects
          .filter((o) => o.kind === "team_spawn")
          .slice(0, 2),
      ],
    };
    const spawns = map.objects.filter((o) => o.kind === "team_spawn");
    spawns[0].position = [2, 0, -6];
    spawns[1].position = [2, 0, 6];
    const mode = {
      ...templates.modes[0],
      name: "Acceptance rules",
      score_limit: 2,
      time_limit: 30,
    };
    let r = await request(
      "/api/content/maps",
      "POST",
      { document: map },
      a.cookie,
    );
    assert.equal(r.status, 200);
    const mv = r.data;
    r = await request(
      "/api/content/modes",
      "POST",
      { document: mode },
      a.cookie,
    );
    assert.equal(r.status, 200);
    const gv = r.data;
    const config = {
      name: "Acceptance lobby",
      max_players: 4,
      visibility: "public",
      map_version: mv.id,
      mode_version: gv.id,
    };
    assert.equal(
      (await request("/api/servers", "POST", config, a.cookie)).status,
      400,
      "drafts cannot host",
    );
    assert.equal(
      (
        await request(
          `/api/content/maps/${mv.id}/publish`,
          "POST",
          null,
          b.cookie,
        )
      ).status,
      403,
    );
    assert.equal(
      (
        await request(
          "/api/content/maps",
          "POST",
          { content_id: mv.content_id, document: map },
          b.cookie,
        )
      ).status,
      403,
    );
    assert.equal(
      (
        await request(
          `/api/content/maps/${mv.id}/publish`,
          "POST",
          null,
          a.cookie,
        )
      ).status,
      200,
    );
    assert.equal(
      (
        await request(
          `/api/content/modes/${gv.id}/publish`,
          "POST",
          null,
          a.cookie,
        )
      ).status,
      200,
    );
    const revision = (
      await request(
        "/api/content/maps",
        "POST",
        {
          content_id: mv.content_id,
          document: { ...map, name: "Revised arena" },
        },
        a.cookie,
      )
    ).data;
    assert.equal(revision.version, 2);
    assert.notEqual(revision.id, mv.id);
    const listed = (await request("/api/content/maps", "GET", null, b.cookie))
      .data;
    assert.equal(
      listed.find((v) => v.id === mv.id).document.name,
      "Acceptance arena",
    );
    assert.ok(
      !listed.some((v) => v.id === revision.id),
      "private drafts stay private",
    );
    assert.equal(
      (
        await request(
          "/api/content/maps",
          "POST",
          { document: { ...map, objects: [] } },
          a.cookie,
        )
      ).status,
      400,
    );
    assert.equal(
      (
        await request(
          "/api/content/modes",
          "POST",
          {
            document: {
              ...mode,
              rules: [
                {
                  event: "player_killed",
                  conditions: [],
                  effects: [{ type: "eval", code: "evil" }],
                },
              ],
            },
          },
          a.cookie,
        )
      ).status,
      400,
    );
    const room = (await request("/api/servers", "POST", config, a.cookie)).data;
    assert.ok(room.id);
    assert.equal(
      (await request(`/api/servers/${room.id}`, "PUT", config, b.cookie))
        .status,
      403,
      "only the owner can configure a server",
    );
    const configured = await request(
      `/api/servers/${room.id}`,
      "PUT",
      { ...config, name: "Configured acceptance lobby", max_players: 2 },
      a.cookie,
    );
    assert.equal(configured.status, 200);
    assert.equal(configured.data.config.max_players, 2);
    assert.equal(configured.data.config.name, "Configured acceptance lobby");
    const second = (
      await request(
        "/api/servers",
        "POST",
        { ...config, name: "Independent room", visibility: "unlisted" },
        b.cookie,
      )
    ).data;
    assert.ok(
      (await request("/api/servers")).data.some((r) => r.id === room.id),
    );
    assert.ok(
      !(await request("/api/servers")).data.some((r) => r.id === second.id),
    );
    const ca = await connect(room.id, a.cookie);
    let cb = await connect(room.id, b.cookie);
    const independent = await connect(second.id, b.cookie);
    await until(
      () => independent.packet?.state.players.length === 1,
      "second room join",
    );
    assert.equal(
      (await request(`/api/servers/${second.id}/start`, "POST", null, b.cookie))
        .status,
      200,
    );
    t.after(() => {
      ca.ws.close();
      cb.ws.close();
      independent.ws.close();
    });
    await until(
      () =>
        ca.packet?.state.players.length === 2 &&
        cb.packet?.state.players.length === 2,
      "replicate joins",
    );
    assert.equal(
      (await request(`/api/servers/${room.id}`, "PUT", config, a.cookie))
        .status,
      400,
      "live participants prevent content replacement",
    );
    await assert.rejects(
      connect(room.id, a.cookie),
      "duplicate joins rejected",
    );
    assert.equal(
      (await request(`/api/servers/${room.id}/start`, "POST", null, b.cookie))
        .status,
      403,
    );
    assert.equal(
      (await request(`/api/servers/${room.id}/start`, "POST", null, a.cookie))
        .status,
      200,
    );
    await until(() => ca.packet?.state.phase === "playing", "match countdown");
    await until(
      () => independent.packet?.state.phase === "playing",
      "two simultaneous active worlds",
    );
    const before = ca.packet.state.players.find(
      (p) => p.id === a.data.id,
    ).position;
    for (let i = 0; i < 25; i++) {
      send(ca, { x: 1 });
      send(cb);
      await sleep(17);
    }
    await until(
      () => ca.packet.state.players.find((p) => p.id === a.data.id).ack >= 20,
      "input acknowledgements",
    );
    const after = ca.packet.state.players.find(
      (p) => p.id === a.data.id,
    ).position;
    assert.ok(after[0] > before[0] + 1, "authoritative movement");
    assert.ok(after[0] < before[0] + 5, "bounded movement");
    for (
      let i = 0;
      i < 210 &&
      ca.packet.state.players.find((p) => p.id === a.data.id).kills === 0;
      i++
    ) {
      const p = ca.packet.state.players.find((p) => p.id === a.data.id),
        q = ca.packet.state.players.find((p) => p.id === b.data.id);
      const dx = q.position[0] - p.position[0],
        dz = q.position[2] - p.position[2];
      send(ca, {
        fire: true,
        yaw: Math.atan2(dx, -dz),
        view_tick: ca.packet.state.tick,
      });
      send(cb);
      await sleep(17);
    }
    await until(
      () => ca.packet.state.players.find((p) => p.id === a.data.id).kills === 1,
      "first kill before match completion",
    );
    const dead = ca.packet.state.players.find((p) => p.id === b.data.id);
    assert.equal(dead.alive, false);
    const respawnAt = dead.respawn_at;
    cb.ws.close();
    await until(
      () => ca.packet.state.players.length === 1,
      "dead player leaves",
    );
    cb = await connect(room.id, b.cookie);
    await until(
      () => ca.packet.state.players.length === 2,
      "same identity reconnects",
    );
    const returning = ca.packet.state.players.find((p) => p.id === b.data.id);
    assert.equal(returning.alive, false, "reconnect does not bypass death");
    assert.equal(returning.deaths, 1);
    assert.equal(returning.team, dead.team);
    assert.equal(returning.respawn_at, respawnAt);
    await until(
      () => ca.packet.state.players.find((p) => p.id === b.data.id).alive,
      "authoritative respawn after reconnect",
    );
    for (let i = 0; i < 600 && ca.packet.state.phase === "playing"; i++) {
      const p = ca.packet.state.players.find((p) => p.id === a.data.id),
        q = ca.packet.state.players.find((p) => p.id === b.data.id);
      send(ca, {
        fire: true,
        reload: p.magazines[0] === 0,
        yaw: Math.atan2(
          q.position[0] - p.position[0],
          p.position[2] - q.position[2],
        ),
        view_tick: ca.packet.state.tick,
      });
      send(cb);
      await sleep(17);
    }
    await until(() => ca.packet.state.phase === "finished", "kill and finish");
    const winner = ca.packet.state.players.find((p) => p.id === a.data.id),
      loser = ca.packet.state.players.find((p) => p.id === b.data.id);
    assert.equal(winner.kills, 2);
    assert.equal(winner.score, 2);
    assert.equal(loser.deaths, 2);
    assert.ok(ca.packet.state.events.some((e) => e.kind === "player_killed"));
    assert.ok(ca.packet.state.events.some((e) => e.kind === "match_over"));
    await until(() => ca.packet.persisted, "durable persistence");
    const result = (await request("/api/results/" + ca.packet.match_id)).data;
    assert.equal(result.map_version, mv.id);
    assert.equal(result.mode_version, gv.id);
    assert.equal(result.participants.length, 2);
    assert.equal(result.state.players.find((p) => p.id === a.data.id).score, 2);
    assert.equal(
      (await request(`/api/servers/${room.id}/start`, "POST", null, a.cookie))
        .status,
      200,
    );
    await until(() => ca.packet.state.phase === "starting", "reuse lobby");
    assert.ok(ca.packet.state.players.every((p) => p.score === 0 && p.alive));
    cb.ws.close();
    await until(
      () => ca.packet.state.players.length === 1,
      "disconnect leaves",
    );
    assert.equal(
      independent.packet.state.phase,
      "playing",
      "first match completion does not finish the other world",
    );
    assert.equal(
      (
        await request(
          `/api/servers/${second.id}/finish`,
          "POST",
          null,
          b.cookie,
        )
      ).status,
      200,
    );
    independent.ws.close();
    await until(
      async () =>
        (await request(`/api/servers/${second.id}`, "GET", null, b.cookie)).data
          .state.players.length === 0,
      "independent disconnect",
    );
    assert.equal(
      (await request(`/api/servers/${second.id}`, "GET", null, b.cookie)).data
        .state.players.length,
      0,
      "independent simulations",
    );
    const bad = await connect(second.id, b.cookie);
    bad.ws.send(JSON.stringify({ seq: 1, x: 99 }));
    await new Promise((resolve) => bad.ws.once("close", resolve));
    console.log("Verified persisted match " + result.match_id);
  },
);
test("authentication, origin and health boundaries", async () => {
  assert.equal((await request("/api/content/maps")).status, 401);
  const local = await fetch(base + "/api/session", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Origin: new URL(base).origin,
    },
    body: JSON.stringify({ name: "Same-origin browser" }),
  });
  assert.equal(local.status, 200, "same-origin browser requests are accepted");
  const r = await fetch(base + "/api/session", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Origin: "https://attacker.invalid",
    },
    body: JSON.stringify({ name: "Nope" }),
  });
  assert.equal(r.status, 403);
  const health = await fetch(base + "/health");
  assert.equal(health.status, 200);
  assert.ok(health.headers.get("content-security-policy"));
  assert.equal(health.headers.get("x-content-type-options"), "nosniff");
});

test("new map documents enforce the construction kit even against hostile clients", async () => {
  const owner = await request("/api/session", "POST", {
    name: "Kit acceptance",
  });
  const { map, kit, markers } = (await request("/api/templates")).data;
  assert.equal(kit.length, 7);
  assert.ok(!markers.some((s) => s.kind === "light"));
  const index = map.objects.findIndex(
    (o) => o.properties.piece === "rectangle_box",
  );
  for (const mutate of [
    (m) => {
      m.grid = false;
    },
    (m) => {
      delete m.grid;
    },
    (m) => {
      m.objects[index].position[0] += 0.25;
    },
    (m) => {
      m.objects[index].scale[0] = 3;
    },
    (m) => {
      m.objects[index].rotation = 0.7;
    },
    (m) => {
      m.objects[index].properties.asset = "container";
    },
    (m) => {
      m.objects[index].properties.piece = null;
    },
  ]) {
    const bad = structuredClone(map);
    mutate(bad);
    assert.equal(
      (
        await request(
          "/api/content/maps",
          "POST",
          { document: bad },
          owner.cookie,
        )
      ).status,
      400,
    );
  }
});
