export const primitives = [
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
  "player_spawn",
  "team_spawn",
  "kill_volume",
  "jump_pad",
  "teleporter_entrance",
  "teleporter_exit",
  "speed_boost",
  "gravity_modifier",
  "health_pickup",
  "shield_pickup",
  "weapon_spawn",
  "ammo_pickup",
  "flag",
  "flag_base",
  "scoring_zone",
  "hill",
];
export const weapons = [
  "pistol",
  "rifle",
  "precision",
  "shotgun",
  "sniper",
  "rocket",
];
const $ = (id) => document.getElementById(id);
const clone = (v) => structuredClone(v);
const title = (s) => s.replaceAll("_", " ");
export class Editor {
  constructor(onChange, notify) {
    this.onChange = onChange;
    this.notify = notify;
    this.map = null;
    this.selected = null;
    this.placing = false;
    this.contentId = null;
    this.version = null;
    this.dirty = false;
    this.drag = false;
    for (const k of primitives) $("primitive").add(new Option(title(k), k));
    for (const k of weapons) $("object-weapon").add(new Option(title(k), k));
    $("place").onclick = () => {
      this.placing = true;
      $("editor-status").textContent =
        "Click the grid to place " + title($("primitive").value);
    };
    $("snap").onchange = () => this.draw();
    $("extent").onchange = () => this.draw();
    $("map-name").oninput = () => {
      this.map.name = $("map-name").value;
      this.changed();
    };
    $("map-sky").oninput = () => {
      this.map.skybox = $("map-sky").value;
      this.changed();
    };
    $("object-list").onchange = () => this.select($("object-list").value);
    for (const id of [
      "px",
      "py",
      "pz",
      "sx",
      "sy",
      "sz",
      "rotation",
      "object-color",
      "object-team",
      "object-weapon",
      "object-target",
      "object-strength",
      "object-respawn",
    ])
      $(id).onchange = () => this.apply();
    $("duplicate").onclick = () => {
      const o = this.object();
      if (!o) return;
      const copy = clone(o);
      copy.id = crypto.randomUUID();
      copy.position[0] += 2;
      this.map.objects.push(copy);
      this.select(copy.id);
      this.changed();
    };
    $("delete").onclick = () => this.remove();
    const grid = $("grid");
    grid.onpointerdown = (e) => {
      const p = this.point(e);
      if (this.placing) {
        this.place(p);
        return;
      }
      const candidates = this.map.objects
        .filter((o) => this.hit(o, p))
        .sort((a, b) => a.scale[0] * a.scale[2] - b.scale[0] * b.scale[2]);
      this.select(candidates[0]?.id);
      this.drag = !!this.object();
      if (this.drag) {
        grid.setPointerCapture(e.pointerId);
        this.dragOffset = [
          this.object().position[0] - p[0],
          this.object().position[2] - p[1],
        ];
      }
    };
    grid.onpointermove = (e) => {
      if (!this.drag) return;
      const p = this.point(e),
        o = this.object();
      o.position[0] = this.snap(p[0] + this.dragOffset[0]);
      o.position[2] = this.snap(p[1] + this.dragOffset[1]);
      this.fields();
      this.changed();
    };
    grid.onpointerup = () => (this.drag = false);
    grid.onpointercancel = () => (this.drag = false);
    window.addEventListener("keydown", (e) => {
      if (
        e.code === "Delete" &&
        document.activeElement === document.body &&
        !$("sandbox").classList.contains("hidden")
      )
        this.remove();
    });
  }
  load(map, contentId = null, version = null) {
    this.map = clone(map);
    this.contentId = contentId;
    this.version = version;
    this.selected = null;
    this.dirty = false;
    $("map-name").value = map.name;
    $("map-sky").value = map.skybox;
    this.select(null);
    this.changed(false);
  }
  object() {
    return this.map?.objects.find((o) => o.id === this.selected);
  }
  snap(x) {
    const snap = +$("snap").value;
    return Math.round(x / snap) * snap;
  }
  point(e) {
    const r = $("grid").getBoundingClientRect(),
      extent = +$("extent").value;
    return [
      this.snap((((e.clientX - r.left) / r.width) * 2 - 1) * extent),
      this.snap((((e.clientY - r.top) / r.height) * 2 - 1) * extent),
    ];
  }
  hit(o, p) {
    const x = p[0] - o.position[0],
      z = p[1] - o.position[2],
      c = Math.cos(o.rotation),
      s = Math.sin(o.rotation);
    return (
      Math.abs(c * x - s * z) <= Math.max(o.scale[0] / 2, 0.6) &&
      Math.abs(s * x + c * z) <= Math.max(o.scale[2] / 2, 0.6)
    );
  }
  place(p) {
    const kind = $("primitive").value;
    const solid = primitives.indexOf(kind) <= 12;
    let scale = solid ? [3, 3, 3] : [2, 2, 2];
    let y = solid ? 1.5 : 0.1;
    if (["floor", "platform", "bridge"].includes(kind)) {
      scale = [8, 1, 8];
      y = -0.5;
    }
    if (kind === "wall" || kind === "barrier") scale = [6, 3, 1];
    if (kind === "ramp" || kind === "stairs" || kind === "wedge") {
      scale = [4, 3, 8];
    }
    if (kind === "hill" || kind === "scoring_zone") {
      scale = [6, 3, 6];
      y = 1.5;
    }
    if (kind === "jump_pad") {
      scale = [3, 0.3, 3];
      y = 0.15;
    }
    const o = {
      id: crypto.randomUUID(),
      kind,
      position: [p[0], y, p[1]],
      rotation: 0,
      scale,
      properties: {
        team:
          kind === "team_spawn" || kind === "flag_base" || kind === "flag"
            ? 0
            : null,
        target: null,
        weapon: "rifle",
        strength: kind === "speed_boost" ? 16 : 14,
        respawn_seconds: 15,
        color: solid ? "#8263ed" : "#c8f788",
      },
    };
    this.map.objects.push(o);
    this.placing = false;
    this.select(o.id);
    this.changed();
  }
  select(id) {
    this.selected = id;
    $("object-empty").classList.toggle("hidden", !!this.object());
    $("object-fields").classList.toggle("hidden", !this.object());
    this.fields();
    this.list();
    this.draw();
  }
  fields() {
    const o = this.object();
    if (!o) return;
    $("object-id").value = o.id;
    ["px", "py", "pz"].forEach((k, i) => ($(k).value = o.position[i]));
    ["sx", "sy", "sz"].forEach((k, i) => ($(k).value = o.scale[i]));
    $("rotation").value = Math.round((o.rotation * 180) / Math.PI);
    $("object-color").value = o.properties.color;
    $("object-team").value = o.properties.team ?? "";
    $("object-weapon").value = o.properties.weapon;
    $("object-target").value = o.properties.target ?? "";
    $("object-strength").value = o.properties.strength;
    $("object-respawn").value = o.properties.respawn_seconds;
  }
  apply() {
    const o = this.object();
    if (!o) return;
    o.position = ["px", "py", "pz"].map((k) => +$(k).value);
    o.scale = ["sx", "sy", "sz"].map((k) => +$(k).value);
    o.rotation = (+$("rotation").value * Math.PI) / 180;
    o.properties = {
      team: $("object-team").value === "" ? null : +$("object-team").value,
      weapon: $("object-weapon").value,
      target: $("object-target").value || null,
      strength: +$("object-strength").value,
      respawn_seconds: +$("object-respawn").value,
      color: $("object-color").value,
    };
    this.changed();
  }
  remove() {
    if (!this.object()) return;
    this.map.objects = this.map.objects.filter((o) => o.id !== this.selected);
    this.select(null);
    this.changed();
  }
  list() {
    if (!this.map) return;
    $("object-list").replaceChildren(
      ...this.map.objects.map((o) => {
        const option = new Option(
          title(o.kind) + " · " + o.id.slice(0, 8),
          o.id,
        );
        option.selected = o.id === this.selected;
        return option;
      }),
    );
  }
  changed(dirty = true) {
    if (dirty) this.dirty = true;
    this.list();
    this.draw();
    $("map-version").textContent = this.version
      ? `Version ${this.version.version} · ${this.version.published ? "published and immutable" : "draft"}${this.dirty ? " · unsaved changes" : ""}`
      : "New map · save a draft or publish when ready";
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.onChange(this.map), 100);
  }
  draw() {
    if (!this.map) return;
    const c = $("grid").getContext("2d"),
      size = 800,
      extent = +$("extent").value,
      ratio = size / (extent * 2);
    c.fillStyle = "#121a2b";
    c.fillRect(0, 0, size, size);
    c.strokeStyle = "#28324a";
    c.lineWidth = 1;
    const spacing = Math.max(1, +$("snap").value) * ratio;
    for (let x = 0; x <= size; x += spacing) {
      c.beginPath();
      c.moveTo(x, 0);
      c.lineTo(x, size);
      c.moveTo(0, x);
      c.lineTo(size, x);
      c.stroke();
    }
    for (const o of [...this.map.objects].sort(
      (a, b) => b.scale[0] * b.scale[2] - a.scale[0] * a.scale[2],
    )) {
      c.save();
      c.translate(
        size / 2 + o.position[0] * ratio,
        size / 2 + o.position[2] * ratio,
      );
      c.rotate(-o.rotation);
      const w = Math.max(o.scale[0] * ratio, 8),
        h = Math.max(o.scale[2] * ratio, 8);
      c.globalAlpha = primitives.indexOf(o.kind) <= 12 ? 0.7 : 0.95;
      c.fillStyle = o.properties.color;
      c.fillRect(-w / 2, -h / 2, w, h);
      c.globalAlpha = 1;
      c.strokeStyle = o.id === this.selected ? "#fff" : "#0008";
      c.lineWidth = o.id === this.selected ? 3 : 1;
      c.strokeRect(-w / 2, -h / 2, w, h);
      if (
        o.kind.includes("spawn") ||
        o.kind.includes("flag") ||
        o.kind === "hill"
      ) {
        c.fillStyle = "#fff";
        c.font = "bold 11px system-ui";
        c.textAlign = "center";
        c.fillText(
          o.kind.includes("spawn") ? "S" : o.kind === "hill" ? "H" : "F",
          0,
          4,
        );
      }
      c.restore();
    }
    $("editor-status").textContent = this.placing
      ? "Click to place"
      : `${this.map.objects.length} objects${this.object() ? " · " + title(this.object().kind) : ""}`;
  }
}
