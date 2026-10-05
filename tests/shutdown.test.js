import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import WebSocket from "ws";

const docker = (...args) => promisify(execFile)("docker", args);
const base = process.env.TEST_URL || "http://127.0.0.1:18081";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

test(
  "SIGTERM closes active clients and persists their match across restart",
  {
    skip: !process.env.SMOKE_CONTAINER,
    timeout: 45000,
  },
  async (t) => {
    let cookie;
    async function request(path, body) {
      const response = await fetch(base + path, {
        method: body ? "POST" : "GET",
        headers: {
          "Content-Type": "application/json",
          ...(cookie ? { Cookie: cookie } : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
      });
      assert.equal(response.status, 200, path);
      cookie ||= response.headers.get("set-cookie")?.split(";")[0];
      return response.json();
    }
    await request("/api/session", { name: "Shutdown witness" });
    const templates = await request("/api/templates");
    const map = await request("/api/content/maps", { document: templates.map });
    const mode = await request("/api/content/modes", {
      document: templates.modes[0],
    });
    await request(`/api/content/maps/${map.id}/publish`, {});
    await request(`/api/content/modes/${mode.id}/publish`, {});
    const room = await request("/api/servers", {
      name: "Shutdown persistence",
      visibility: "unlisted",
      max_players: 2,
      map_version: map.id,
      mode_version: mode.id,
    });
    const socket = new WebSocket(
      base.replace(/^http/, "ws") + `/api/servers/${room.id}/ws`,
      { headers: { Cookie: cookie } },
    );
    t.after(() => socket.terminate());
    let state;
    socket.on("message", (message) => {
      state = JSON.parse(message).state;
    });
    await new Promise((resolve, reject) => {
      socket.once("open", resolve);
      socket.once("error", reject);
    });
    const match = await request(`/api/servers/${room.id}/start`, {});
    const deadline = Date.now() + 10000;
    while (state?.phase !== "playing" && Date.now() < deadline) await sleep(50);
    assert.equal(state?.phase, "playing");
    const closed = new Promise((resolve) => socket.once("close", resolve));
    await docker("stop", "--time", "20", process.env.SMOKE_CONTAINER);
    await closed;
    assert.equal(
      (
        await docker(
          "inspect",
          "-f",
          "{{.State.ExitCode}}",
          process.env.SMOKE_CONTAINER,
        )
      ).stdout.trim(),
      "0",
    );
    await docker("start", process.env.SMOKE_CONTAINER);
    for (let i = 0; i < 100; i++) {
      try {
        if ((await fetch(base + "/health")).ok) break;
      } catch {}
      await sleep(100);
    }
    const result = await request(`/api/results/${match.match_id}`);
    assert.equal(result.state.phase, "finished");
    assert.equal(result.participants.length, 1);
    assert.equal(result.participants[0].name, "Shutdown witness");
    console.log("Verified shutdown result " + match.match_id);
  },
);
