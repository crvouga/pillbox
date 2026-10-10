import { test } from "node:test";
import assert from "node:assert/strict";
import { anchorOf, setAnchor, worldSize } from "../web/editor.js";
// Exercises the same catalog consumed by the browser, including each quarter turn.
const base = process.env.TEST_URL || "http://127.0.0.1:8080";
test("editor moves and rotates every kit piece by its faces and never changes its dimensions", async () => {
  const { kit, markers } = await (await fetch(base + "/api/templates")).json();
  for (const spec of kit) {
    const o = {
      kind: spec.kind,
      scale: [...spec.size],
      rotation: 0,
      position: [0, 0, 0],
    };
    for (let turn = 0; turn < 4; turn++) {
      o.rotation = (turn * Math.PI) / 2;
      setAnchor(o, [-7.7, 7.2, 12.3], markers);
      assert.deepEqual(anchorOf(o, markers), [-8, 7, 12]);
      assert.deepEqual(o.scale, spec.size);
      assert.deepEqual(
        worldSize(o),
        turn % 2 ? [spec.size[2], spec.size[1], spec.size[0]] : spec.size,
      );
      const next = anchorOf(o, markers);
      next[0] += worldSize(o)[0];
      const duplicate = structuredClone(o);
      setAnchor(duplicate, next, markers);
      assert.equal(
        duplicate.position[0] - worldSize(duplicate)[0] / 2,
        o.position[0] + worldSize(o)[0] / 2,
      );
    }
  }
  for (const spec of markers) {
    const o = {
      kind: spec.kind,
      scale: spec.size,
      rotation: 0,
      position: [0, 0, 0],
    };
    setAnchor(o, [0, 8, 4], markers);
    assert.deepEqual(anchorOf(o, markers), [0, 8, 4]);
    assert.equal(o.position[1], spec.feet ? 8 : 8 + spec.size[1] / 2);
  }
});

test("radar fits every rotated piece of the expanded arenas without clipping", async () => {
  const { radarProjection } = await import("../web/hud.js");
  const { maps } = await (await fetch(base + "/api/templates")).json();
  for (const map of maps) {
    const { ratio, center } = radarProjection(map, 160);
    for (const o of map.objects.filter((o) => o.properties.piece)) {
      const size = worldSize(o);
      for (const sign of [-1, 1]) {
        const x =
          80 + (o.position[0] + (sign * size[0]) / 2 - center[0]) * ratio;
        const z =
          80 + (o.position[2] + (sign * size[2]) / 2 - center[1]) * ratio;
        assert.ok(
          x >= 7.99 && x <= 152.01 && z >= 7.99 && z <= 152.01,
          `${map.name}: clipped ${o.id}`,
        );
      }
    }
  }
});
