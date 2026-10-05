import { test } from "node:test";
import assert from "node:assert/strict";
import WebSocket from "ws";
const base = process.env.TEST_URL || "http://127.0.0.1:8080";
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, ms = 15000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const result = await fn();
    if (result) return result;
    await sleep(40);
  }
  throw Error("Practice acceptance timed out");
}
test(
  "practice opponents obey room capacity, fight, respawn and persist",
  { timeout: 45000 },
  async (t) => {
    const owner = await request("/api/session", "POST", {
      name: "Practice acceptance",
    });
    assert.equal(owner.status, 200);
    const templates = (await request("/api/templates")).data;
    assert.equal(templates.maps.length, 3);
    const map = {
      ...templates.map,
      name: "Practice acceptance map",
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
    const mv = (
      await request(
        "/api/content/maps",
        "POST",
        { document: map },
        owner.cookie,
      )
    ).data;
    const gv = (
      await request(
        "/api/content/modes",
        "POST",
        {
          document: {
            ...templates.modes[0],
            name: "Practice acceptance mode",
            score_limit: 3,
            time_limit: 30,
            respawn_seconds: 1,
          },
        },
        owner.cookie,
      )
    ).data;
    for (const [kind, id] of [
      ["maps", mv.id],
      ["modes", gv.id],
    ])
      assert.equal(
        (
          await request(
            `/api/content/${kind}/${id}/publish`,
            "POST",
            null,
            owner.cookie,
          )
        ).status,
        200,
      );
    const config = {
      name: "Practice acceptance room",
      visibility: "unlisted",
      max_players: 4,
      map_version: mv.id,
      mode_version: gv.id,
      practice_bots: 3,
    };
    assert.equal(
      (
        await request(
          "/api/servers",
          "POST",
          { ...config, practice_bots: 5 },
          owner.cookie,
        )
      ).status,
      400,
    );
    assert.equal(
      (
        await request(
          "/api/servers",
          "POST",
          { ...config, max_players: 3 },
          owner.cookie,
        )
      ).status,
      400,
    );
    const r = await request("/api/servers", "POST", config, owner.cookie);
    assert.equal(r.status, 200);
    const room = r.data;
    const ws = new WebSocket(
      base.replace(/^http/, "ws") + `/api/servers/${room.id}/ws`,
      { headers: { Cookie: owner.cookie } },
    );
    t.after(() => ws.close());
    let packet;
    const events = new Map();
    let seq = 0;
    ws.on("message", (data) => {
      packet = JSON.parse(data);
      for (const e of packet.state.events) events.set(e.id, e);
    });
    await new Promise((resolve, reject) => {
      ws.once("open", resolve);
      ws.once("error", reject);
    });
    const heartbeat = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN)
        ws.send(
          JSON.stringify({
            seq: ++seq,
            x: 0,
            z: 0,
            yaw: 0,
            pitch: 0,
            view_tick: packet?.state.tick || 0,
          }),
        );
    }, 50);
    t.after(() => clearInterval(heartbeat));
    await until(() => packet?.state.players.length === 4);
    assert.equal(
      packet.state.players.filter((p) => p.id.startsWith("bot:")).length,
      3,
    );
    const extra = await request("/api/session", "POST", {
      name: "Capacity check",
    });
    const denied = new WebSocket(
      base.replace(/^http/, "ws") + `/api/servers/${room.id}/ws`,
      { headers: { Cookie: extra.cookie } },
    );
    await new Promise((resolve, reject) => {
      denied.once("unexpected-response", (_, res) => {
        assert.equal(res.statusCode, 400);
        res.destroy();
        resolve();
      });
      denied.once("open", () => {
        denied.close();
        reject(Error("Bots must occupy real player slots"));
      });
      denied.once("error", resolve);
    });
    assert.equal(
      (
        await request(
          `/api/servers/${room.id}/start`,
          "POST",
          null,
          owner.cookie,
        )
      ).status,
      200,
    );
    await until(() =>
      [...events.values()].some(
        (e) => e.kind === "shot" && e.actor?.startsWith("bot:"),
      ),
    );
    await until(() =>
      [...events.values()].some(
        (e) => e.kind === "player_killed" && e.actor?.startsWith("bot:"),
      ),
    );
    if (packet.state.phase !== "finished")
      assert.equal(
        (
          await request(
            `/api/servers/${room.id}/finish`,
            "POST",
            null,
            owner.cookie,
          )
        ).status,
        200,
      );
    await until(() => packet?.persisted);
    const result = await request(`/api/results/${packet.match_id}`);
    assert.equal(result.status, 200);
    assert.equal(
      result.data.participants.filter((p) => p.id.startsWith("bot:")).length,
      3,
    );
    assert.ok(
      result.data.participants.some(
        (p) => p.id.startsWith("bot:") && p.kills > 0,
      ),
    );
    assert.ok(
      [...events.values()]
        .filter((e) => e.kind === "shot")
        .every((e) => e.endpoint?.length === 3),
    );
  },
);
