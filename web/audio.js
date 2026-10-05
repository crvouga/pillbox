// Offline assets only: no runtime API key, paid request, or browser TTS.
const root = "/assets/audio/";
export const shots = {
  pistol: "laserSmall_000",
  rifle: "laserSmall_001",
  precision: "laserLarge_000",
  shotgun: "laserRetro_000",
  sniper: "laserLarge_001",
  rocket: "thrusterFire_000",
};
export const actions = {
  footstep: "footstep_concrete_000",
  jump: "forceField_000",
  jump_pad: "forceField_000",
  land: "impactMetal_heavy_000",
  reload: "doorOpen_000",
  reload_complete: "doorClose_000",
  weapon_switch: "impactMetal_light_001",
  dry_fire: "impactPlate_light_000",
  hit: "impactMetal_000",
  shield_broken: "forceField_000",
  shield_recharge: "forceField_000",
  respawn: "doorOpen_000",
  teleport: "forceField_000",
  weapon_pickup: "computerNoise_000",
  health_pickup: "forceField_000",
  shield_pickup: "forceField_000",
  ammo_pickup: "impactMetal_light_001",
  player_killed: "lowFrequency_explosion_000",
  explosion: "lowFrequency_explosion_000",
  flag_taken: "computerNoise_000",
  flag_dropped: "impactMetal_light_001",
  flag_returned: "forceField_000",
  flag_captured: "computerNoise_000",
  hill_controlled: "computerNoise_000",
  hill_contested: "computerNoise_000",
};
const fallback = {
  match_starting: "prepare_yourself",
  match_started: "fight",
  match_over: "game_over",
  victory: "victory",
  defeat: "mission_failed",
  double_kill: "multi_kill",
  triple_kill: "multi_kill",
  killing_spree: "multi_kill",
  one_minute_remaining: "hurry_up",
  thirty_seconds_remaining: "hurry_up",
  flag_taken: "power_up",
  flag_captured: "objective_achieved",
  hill_controlled: "objective_achieved",
};
export class ArenaAudio {
  constructor() {
    this.cache = new Map();
    this.enabled = true;
    this.volume = 0.7;
    this.voiceEnabled = true;
    this.ambience = [];
    this.voice = null;
    this.metrics = { effects: 0, voices: 0, failed: [] };
    this.generated = {};
    fetch(root + "voice/manifest.json")
      .then((r) => (r.ok ? r.json() : {}))
      .then((m) => (this.generated = m))
      .catch(() => {});
  }
  async unlock() {
    if (!this.ctx) {
      this.ctx = new (window.AudioContext || window.webkitAudioContext)();
      this.master = this.ctx.createGain();
      this.fx = this.ctx.createGain();
      this.voiceBus = this.ctx.createGain();
      this.fx.connect(this.master);
      this.voiceBus.connect(this.master);
      this.master.connect(this.ctx.destination);
      this.voiceBus.gain.value = 0.8;
      this.master.gain.value = this.enabled ? this.volume : 0;
    }
    await this.ctx.resume();
  }
  async buffer(path) {
    if (!this.cache.has(path))
      this.cache.set(
        path,
        fetch(path)
          .then((r) => {
            if (!r.ok) throw Error("Missing audio " + path);
            return r.arrayBuffer();
          })
          .then((b) => this.ctx.decodeAudioData(b))
          .catch((e) => {
            this.metrics.failed.push(path);
            console.warn(e.message);
            return null;
          }),
      );
    return this.cache.get(path);
  }
  async play(
    name,
    {
      position = null,
      volume = 0.45,
      pitch = 1,
      loop = false,
      voice = false,
    } = {},
  ) {
    if (!this.ctx || this.ctx.state !== "running" || !this.enabled) return null;
    const buffer = await this.buffer(
      name.startsWith("/") ? name : root + name + ".m4a",
    );
    if (!buffer || !this.enabled || this.ctx.state !== "running") return null;
    const source = this.ctx.createBufferSource();
    source.buffer = buffer;
    source.loop = loop;
    source.playbackRate.value = pitch;
    const gain = this.ctx.createGain();
    gain.gain.value = volume;
    source.connect(gain);
    const bus = voice ? this.voiceBus : this.fx;
    let p = null;
    if (position) {
      p = this.ctx.createPanner();
      p.panningModel = "HRTF";
      p.distanceModel = "inverse";
      p.refDistance = 5;
      p.maxDistance = 55;
      p.rolloffFactor = 1.5;
      p.positionX.value = position[0];
      p.positionY.value = position[1];
      p.positionZ.value = position[2];
      gain.connect(p);
      p.connect(bus);
    } else gain.connect(bus);
    source.addEventListener(
      "ended",
      () => {
        source.disconnect();
        gain.disconnect();
        p?.disconnect();
      },
      { once: true },
    );
    source.start();
    if (voice) this.metrics.voices++;
    else this.metrics.effects++;
    return source;
  }
  event(e, localId) {
    const name = e.kind === "shot" ? shots[e.weapon] : actions[e.kind];
    if (!name) return;
    const own = e.actor === localId;
    const foot = e.kind === "footstep";
    this.play(name, {
      position: own ? null : e.position,
      volume: foot ? (own ? 0.14 : 0.3) : e.kind === "explosion" ? 0.65 : 0.42,
      pitch: foot ? 0.94 + (e.id % 5) * 0.025 : 1,
    });
  }
  async announce(kind) {
    if (!this.voiceEnabled || !this.enabled) return;
    const path =
      this.generated[kind] ||
      (fallback[kind] ? root + "voice/" + fallback[kind] + ".m4a" : null);
    if (!path) return;
    // Spree/warning calls do not interrupt a higher-priority match transition.
    if (
      this.voice &&
      !["match_started", "victory", "defeat", "match_over"].includes(kind)
    )
      return;
    const generation = (this.voiceGeneration || 0) + 1;
    this.voiceGeneration = generation;
    this.voice?.stop();
    this.fx?.gain.setTargetAtTime(0.5, this.ctx.currentTime, 0.06);
    const source = await this.play(path, { volume: 0.85, voice: true });
    if (this.voiceGeneration !== generation) {
      source?.stop();
      return;
    }
    this.voice = source;
    if (this.voice) {
      const v = this.voice;
      v.onended = () => {
        v.disconnect();
        if (this.voice === v) {
          this.voice = null;
          this.fx.gain.setTargetAtTime(1, this.ctx.currentTime, 0.15);
        }
      };
    }
  }
  listener(p) {
    if (!this.ctx || !p) return;
    const l = this.ctx.listener;
    if (l.positionX) {
      l.positionX.value = p.position[0];
      l.positionY.value = p.position[1] + 1.5;
      l.positionZ.value = p.position[2];
      l.forwardX.value = Math.sin(p.yaw) * Math.cos(p.pitch);
      l.forwardY.value = Math.sin(p.pitch);
      l.forwardZ.value = -Math.cos(p.yaw) * Math.cos(p.pitch);
      l.upX.value = 0;
      l.upY.value = 1;
      l.upZ.value = 0;
    }
  }
  async atmosphere(active) {
    const generation = (this.atmosphereGeneration || 0) + 1;
    this.atmosphereGeneration = generation;
    this.ambience.forEach((s) => s.stop());
    this.ambience = [];
    if (active && this.ctx) {
      for (const [name, volume] of [
        ["spaceEngineLow_000", 0.08],
        ["engineCircular_000", 0.025],
      ]) {
        const s = await this.play(name, { volume, loop: true });
        if (s) {
          if (this.atmosphereGeneration === generation) this.ambience.push(s);
          else s.stop();
        }
      }
    }
  }
  setEnabled(value) {
    this.enabled = value;
    if (this.master) this.master.gain.value = value ? this.volume : 0;
    if (!value) {
      this.voiceGeneration = (this.voiceGeneration || 0) + 1;
      this.voice?.stop();
      this.voice = null;
      if (this.fx) this.fx.gain.value = 1;
    }
  }
  setVolume(value) {
    this.volume = value;
    if (this.master) this.master.gain.value = this.enabled ? value : 0;
  }
}
