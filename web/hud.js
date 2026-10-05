const names = {
  weapon_spawn: "WEAPON",
  health_pickup: "MEDKIT",
  shield_pickup: "SHIELD CELL",
  ammo_pickup: "AMMO",
  jump_pad: "GRAVITY LIFT",
  teleporter_entrance: "PORTAL",
  teleporter_exit: "PORTAL EXIT",
  hill: "CONTROL POINT",
  flag_base: "FLAG BASE",
  team_spawn: "SPAWN",
};
const specials = new Set(Object.keys(names));
const solids = new Set([
  "box",
  "wall",
  "floor",
  "platform",
  "ramp",
  "wedge",
  "arch",
  "pillar",
  "cylinder",
  "sphere",
  "stairs",
  "bridge",
  "barrier",
]);
const sub = (a, b) => a.map((v, i) => v - b[i]);
const dot = (a, b) => a.reduce((s, v, i) => s + v * b[i], 0);
function hit(o, origin, dir, max = 100) {
  const c = Math.cos(-o.rotation),
    s = Math.sin(-o.rotation),
    v = sub(origin, o.position);
  const p = [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]],
    d = [c * dir[0] + s * dir[2], dir[1], -s * dir[0] + c * dir[2]];
  let near = 0,
    far = max;
  for (let i = 0; i < 3; i++) {
    const h = o.scale[i] / 2;
    if (Math.abs(d[i]) < 0.00001) {
      if (Math.abs(p[i]) > h) return null;
    } else {
      const a = (-h - p[i]) / d[i],
        b = (h - p[i]) / d[i];
      near = Math.max(near, Math.min(a, b));
      far = Math.min(far, Math.max(a, b));
    }
  }
  if (near > far) return null;
  if (o.kind === "arch") {
    const x = p[0] + d[0] * near,
      y = p[1] + d[1] * near;
    if (Math.abs(x) < o.scale[0] * 0.325 && y < o.scale[1] * 0.275) return null;
  }
  if (o.kind === "ramp" || o.kind === "wedge") {
    for (let t = near; t <= far; t += 0.1) {
      const z = p[2] + d[2] * t,
        y = p[1] + d[1] * t;
      const top =
        -o.scale[1] / 2 + (o.scale[1] * (z + o.scale[2] / 2)) / o.scale[2];
      if (y <= top) return t;
    }
    return null;
  }
  return near;
}
export class ArenaHUD {
  constructor() {
    this.nodes = new Map();
    this.map = null;
    this.context = document.getElementById("object-context");
    this.root = document.getElementById("object-labels");
    this.radar = document.getElementById("radar");
    this.last = 0;
  }
  setMap(map) {
    this.map = map;
    this.root.replaceChildren();
    this.nodes.clear();
  }
  update(p, state, time, ads = false, teams = false) {
    if (!p || !this.map || !p.alive) {
      this.root.replaceChildren();
      this.nodes.clear();
      this.context.textContent = "";
      return;
    }
    if (time - this.last < 50) return;
    this.last = time;
    const eye = [p.position[0], p.position[1] + 1.5, p.position[2]],
      forward = [
        Math.sin(p.yaw) * Math.cos(p.pitch),
        Math.sin(p.pitch),
        -Math.cos(p.yaw) * Math.cos(p.pitch),
      ],
      right = [Math.cos(p.yaw), 0, Math.sin(p.yaw)],
      up = [
        -Math.sin(p.yaw) * Math.sin(p.pitch),
        Math.cos(p.pitch),
        Math.cos(p.yaw) * Math.sin(p.pitch),
      ];
    const canvas = document.getElementById("game"),
      rect = canvas.getBoundingClientRect(),
      f = rect.height / (2 * Math.tan(((ads ? 50 : 85) * Math.PI) / 360));
    const project = (position) => {
      const d = sub(position, eye),
        depth = dot(d, forward);
      if (depth < 0.2) return null;
      return [
        rect.left + rect.width / 2 + (dot(d, right) * f) / depth,
        rect.top + rect.height / 2 - (dot(d, up) * f) / depth,
        Math.hypot(...d),
      ];
    };
    const blocked = (position, id) => {
      const d = sub(position, eye),
        len = Math.hypot(...d),
        dir = d.map((v) => v / len);
      return this.map.objects.some(
        (o) =>
          o.id !== id &&
          solids.has(o.kind) &&
          hit(o, eye, dir, len - 0.3) !== null,
      );
    };
    let closest = null,
      range = 30;
    for (const o of this.map.objects) {
      const t = hit(o, eye, forward, range);
      if (t !== null && t < range) {
        range = t;
        closest = o;
      }
    }
    this.context.textContent = closest
      ? `${closest.properties.label || names[closest.kind] || closest.kind.replaceAll("_", " ").toUpperCase()} · ${Math.round(range)}m`
      : "";
    const labels = [];
    for (const o of this.map.objects) {
      if (!specials.has(o.kind) || o.kind === "team_spawn") continue;
      const until = state?.pickups?.[o.id];
      if (until > state.tick) continue;
      const position = [...o.position];
      position[1] += o.kind === "weapon_spawn" ? 0.65 : 0.5;
      const screen = project(position);
      if (
        !screen ||
        screen[2] > 40 ||
        screen[0] < 30 ||
        screen[0] > innerWidth - 30 ||
        screen[1] < 60 ||
        screen[1] > innerHeight - 110 ||
        blocked(position, o.id)
      )
        continue;
      let name =
        o.kind === "weapon_spawn"
          ? o.properties.weapon.toUpperCase()
          : o.properties.label || names[o.kind];
      labels.push({ id: o.id, name, screen, kind: o.kind });
    }
    for (const q of state?.players || []) {
      if (q.id === p.id || !q.alive) continue;
      const pos = [q.position[0], q.position[1] + 2.2, q.position[2]],
        screen = project(pos);
      if (screen && screen[2] < 40 && !blocked(pos, ""))
        labels.push({
          id: q.id,
          name: q.name,
          screen,
          kind: teams && q.team === p.team ? "friend" : "enemy",
        });
    }
    labels.sort((a, b) => a.screen[2] - b.screen[2]);
    const alive = new Set();
    for (const l of labels.slice(0, 12)) {
      let el = this.nodes.get(l.id);
      if (!el) {
        el = document.createElement("div");
        el.className = "world-label " + l.kind;
        this.root.append(el);
        this.nodes.set(l.id, el);
      }
      el.textContent = `${l.name}  ${Math.round(l.screen[2])}m`;
      el.style.transform = `translate(${Math.round(l.screen[0])}px,${Math.round(l.screen[1])}px) translate(-50%,-100%)`;
      alive.add(l.id);
    }
    for (const [id, el] of this.nodes)
      if (!alive.has(id)) {
        el.remove();
        this.nodes.delete(id);
      }
    const ctx = this.radar.getContext("2d"),
      size = this.radar.width;
    ctx.clearRect(0, 0, size, size);
    ctx.fillStyle = "#0c1722aa";
    ctx.fillRect(0, 0, size, size);
    ctx.strokeStyle = "#60748855";
    ctx.lineWidth = 1;
    for (const o of this.map.objects.filter(
      (o) => solids.has(o.kind) && o.kind !== "floor",
    )) {
      ctx.save();
      ctx.translate(size / 2 + o.position[0] * 2, size / 2 + o.position[2] * 2);
      ctx.rotate(-o.rotation);
      ctx.strokeRect(-o.scale[0], -o.scale[2], o.scale[0] * 2, o.scale[2] * 2);
      ctx.restore();
    }
    for (const o of this.map.objects.filter(
      (o) => o.kind === "weapon_spawn" || o.kind === "hill",
    )) {
      ctx.fillStyle = o.kind === "hill" ? "#7fe8e8" : "#ffc879";
      ctx.fillRect(
        size / 2 + o.position[0] * 2 - 2,
        size / 2 + o.position[2] * 2 - 2,
        4,
        4,
      );
    }
    ctx.save();
    ctx.translate(size / 2 + p.position[0] * 2, size / 2 + p.position[2] * 2);
    ctx.rotate(p.yaw);
    ctx.fillStyle = "#c8f788";
    ctx.beginPath();
    ctx.moveTo(0, -5);
    ctx.lineTo(-4, 4);
    ctx.lineTo(4, 4);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }
}
