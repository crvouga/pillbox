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
export const snap = (x, step = 4) => Math.round(x / step) * step;
export function worldSize(o) {
  return Math.round(o.rotation / (Math.PI / 2)) % 2
    ? [o.scale[2], o.scale[1], o.scale[0]]
    : [...o.scale];
}
export function anchorOf(o, markers) {
  const size = worldSize(o),
    feet = markers.find((m) => m.kind === o.kind)?.feet;
  return o.position.map((v, i) => v - (i === 1 && feet ? 0 : size[i] / 2));
}
export function setAnchor(o, anchor, markers) {
  const size = worldSize(o),
    feet = markers.find((m) => m.kind === o.kind)?.feet;
  o.position = anchor.map(
    (v, i) => snap(v, i === 1 ? 1 : 4) + (i === 1 && feet ? 0 : size[i] / 2),
  );
}
export class Editor {
  constructor(onChange, notify) {
    Object.assign(this, {
      onChange,
      notify,
      map: null,
      selected: null,
      placing: null,
      contentId: null,
      version: null,
      dirty: false,
      drag: false,
      kit: [],
      markers: [],
    });
    for (const k of weapons) $("object-weapon").add(new Option(title(k), k));
    $("place").onclick = () => {
      this.placing = "piece";
      this.draw();
    };
    $("place-marker").onclick = () => {
      this.placing = "marker";
      this.draw();
    };
    for (const id of ["extent", "layer", "all-layers"])
      $(id).onchange = () => this.draw();
    $("map-name").oninput = () => {
      if (this.editable()) {
        this.map.name = $("map-name").value;
        this.changed();
      }
    };
    $("map-sky").oninput = () => {
      if (this.editable()) {
        this.map.skybox = $("map-sky").value;
        this.changed();
      }
    };
    $("object-list").onchange = () => this.select($("object-list").value);
    for (const id of [
      "px",
      "py",
      "pz",
      "rotation",
      "object-color",
      "object-label",
      "object-team",
      "object-weapon",
      "object-target",
      "object-strength",
      "object-respawn",
    ])
      $(id).onchange = () => this.apply();
    $("duplicate").onclick = () => {
      const o = this.object();
      if (!o || !this.editable()) return;
      const copy = clone(o),
        a = anchorOf(o, this.markers);
      copy.id = crypto.randomUUID();
      a[0] += worldSize(o)[0];
      setAnchor(copy, a, this.markers);
      this.map.objects.push(copy);
      this.select(copy.id);
      this.changed();
    };
    $("delete").onclick = () => this.remove();
    const grid = $("grid");
    grid.onpointerdown = (e) => {
      if (!this.map) return;
      const p = this.point(e);
      if (this.placing) {
        this.place(p);
        return;
      }
      const candidates = this.map.objects
        .filter((o) => this.onLayer(o) && this.hit(o, p))
        .sort(
          (a, b) =>
            Number(!!a.properties.piece) - Number(!!b.properties.piece) ||
            b.position[1] - a.position[1],
        );
      this.select(candidates[0]?.id);
      this.drag = !!this.object() && this.map.grid;
      if (this.drag) {
        grid.setPointerCapture(e.pointerId);
        const a = anchorOf(this.object(), this.markers);
        this.dragOffset = [a[0] - p[0], a[2] - p[1]];
      }
    };
    grid.onpointermove = (e) => {
      if (!this.drag) return;
      const p = this.point(e),
        o = this.object(),
        a = anchorOf(o, this.markers);
      a[0] = p[0] + this.dragOffset[0];
      a[2] = p[1] + this.dragOffset[1];
      setAnchor(o, a, this.markers);
      this.fields();
      this.changed();
    };
    grid.onpointerup = grid.onpointercancel = () => {
      this.drag = false;
    };
    window.addEventListener("keydown", (e) => {
      if (
        e.code === "Delete" &&
        document.activeElement === document.body &&
        !$("sandbox").classList.contains("hidden")
      )
        this.remove();
    });
  }
  setKit(kit, markers) {
    this.kit = kit;
    this.markers = markers;
    $("primitive").replaceChildren(
      ...kit.map(
        (s) =>
          new Option(`${title(s.piece)} · ${s.size.join(" × ")} m`, s.piece),
      ),
    );
    $("marker").replaceChildren(
      ...markers.map((s) => new Option(title(s.kind), s.kind)),
    );
  }
  editable() {
    if (this.map?.grid) return true;
    this.notify(
      "Historical maps are read-only. Start a new arena using the grid kit.",
    );
    return false;
  }
  load(map, contentId = null, version = null) {
    Object.assign(this, {
      map: clone(map),
      contentId,
      version,
      selected: null,
      dirty: false,
      placing: null,
      drag: false,
    });
    $("map-name").value = map.name;
    $("map-sky").value = map.skybox;
    this.select(null);
    this.changed(false);
  }
  object() {
    return this.map?.objects.find((o) => o.id === this.selected);
  }
  point(e) {
    const r = $("grid").getBoundingClientRect(),
      extent = +$("extent").value;
    return [
      (((e.clientX - r.left) / r.width) * 2 - 1) * extent,
      (((e.clientY - r.top) / r.height) * 2 - 1) * extent,
    ];
  }
  onLayer(o) {
    if ($("all-layers").checked) return true;
    const base = anchorOf(o, this.markers)[1],
      layer = Math.round(+$("layer").value);
    return (
      Math.abs(base - layer) < 0.01 ||
      (o.kind === "floor" && Math.abs(base + 1 - layer) < 0.01)
    );
  }
  hit(o, p) {
    const size = worldSize(o);
    return (
      Math.abs(p[0] - o.position[0]) <= size[0] / 2 &&
      Math.abs(p[1] - o.position[2]) <= size[2] / 2
    );
  }
  place(p) {
    if (!this.editable()) {
      this.placing = null;
      return;
    }
    const piece = this.placing === "piece",
      spec = (piece ? this.kit : this.markers).find((s) =>
        piece ? s.piece === $("primitive").value : s.kind === $("marker").value,
      );
    const o = {
      id: crypto.randomUUID(),
      kind: spec.kind,
      position: [0, 0, 0],
      rotation: 0,
      scale: [...spec.size],
      properties: {
        piece: piece ? spec.piece : null,
        team: ["team_spawn", "flag", "flag_base"].includes(spec.kind)
          ? 0
          : null,
        target: null,
        weapon: "rifle",
        strength: 14,
        respawn_seconds: 15,
        color: piece ? "#87949c" : "#dac99c",
        label: "",
        asset: null,
      },
    };
    // The clicked cell is the lower corner. All dimensions remain fixed.
    setAnchor(
      o,
      [
        Math.floor(p[0] / 4) * 4,
        Math.round(+$("layer").value),
        Math.floor(p[1] / 4) * 4,
      ],
      this.markers,
    );
    this.map.objects.push(o);
    this.placing = null;
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
    ["px", "py", "pz"].forEach(
      (k, i) => ($(k).value = anchorOf(o, this.markers)[i]),
    );
    ["sx", "sy", "sz"].forEach((k, i) => ($(k).value = worldSize(o)[i]));
    $("rotation").value = snap((o.rotation * 180) / Math.PI, 90);
    $("object-color").value = o.properties.color;
    $("object-label").value = o.properties.label || "";
    $("object-team").value = o.properties.team ?? "";
    $("object-weapon").value = o.properties.weapon;
    $("object-target").value = o.properties.target ?? "";
    $("object-strength").value = o.properties.strength;
    $("object-respawn").value = o.properties.respawn_seconds;
  }
  apply() {
    const o = this.object();
    if (!o || !this.editable()) return;
    o.rotation =
      ((((snap(+$("rotation").value, 90) % 360) + 360) % 360) * Math.PI) / 180;
    setAnchor(
      o,
      ["px", "py", "pz"].map((k) => +$(k).value),
      this.markers,
    );
    Object.assign(o.properties, {
      team: $("object-team").value === "" ? null : +$("object-team").value,
      weapon: $("object-weapon").value,
      target: $("object-target").value || null,
      strength: +$("object-strength").value,
      respawn_seconds: +$("object-respawn").value,
      color: $("object-color").value,
      label: $("object-label").value,
      asset: null,
    });
    this.fields();
    this.changed();
  }
  remove() {
    if (!this.object() || !this.editable()) return;
    this.map.objects = this.map.objects.filter((o) => o.id !== this.selected);
    this.select(null);
    this.changed();
  }
  list() {
    if (!this.map) return;
    $("object-list").replaceChildren(
      ...this.map.objects.map((o) => {
        const option = new Option(
          `${title(o.properties.piece || o.kind)} · ${o.id}`,
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
    for (
      let cell = Math.ceil(-extent / 4);
      cell <= Math.floor(extent / 4);
      cell++
    ) {
      const x = size / 2 + cell * 4 * ratio;
      c.beginPath();
      c.moveTo(x, 0);
      c.lineTo(x, size);
      c.moveTo(0, x);
      c.lineTo(size, x);
      c.stroke();
    }
    for (const o of [...this.map.objects].sort(
      (a, b) => a.position[1] - b.position[1],
    )) {
      c.save();
      c.translate(
        size / 2 + o.position[0] * ratio,
        size / 2 + o.position[2] * ratio,
      );
      c.rotate(-o.rotation);
      const w = o.scale[0] * ratio,
        h = o.scale[2] * ratio;
      c.globalAlpha = this.onLayer(o) ? 0.85 : 0.08;
      c.fillStyle = o.properties.color;
      c.fillRect(-w / 2, -h / 2, w, h);
      c.strokeStyle = o.id === this.selected ? "#fff" : "#0008";
      c.lineWidth = o.id === this.selected ? 3 : 1;
      c.strokeRect(-w / 2, -h / 2, w, h);
      if (!o.properties.piece && this.onLayer(o)) {
        c.fillStyle = "#fff";
        c.font = "bold 11px system-ui";
        c.textAlign = "center";
        c.fillText(
          o.kind.includes("spawn")
            ? "S"
            : o.kind === "hill"
              ? "H"
              : o.kind.includes("flag")
                ? "F"
                : "•",
          0,
          4,
        );
      }
      c.restore();
    }
    $("editor-status").textContent = this.placing
      ? `Click a cell to place ${title(this.placing === "piece" ? $("primitive").value : $("marker").value)}`
      : `${this.map.objects.length} pieces / markers · 4 m grid${this.object() ? " · " + title(this.object().properties.piece || this.object().kind) : ""}`;
  }
}
