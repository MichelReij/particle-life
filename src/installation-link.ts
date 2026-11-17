// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Instellingen naar de installatie (native simulatie op de Ubuntu) en terug:
// pagina → dev-server (/espnow/…, espnow-kit/tools/espnow_http.js) → OTA-master
// via USB → ESP-NOW → liaison → simulatie, en het antwoord dezelfde weg terug.
// Alleen lokaal: zonder dat eindpunt (michelreij.nl) tonen de panelen niets extra.
//
// Pakketformaat: zie settings_protocol.rs — hier dezelfde bytes:
//   [msgType u16 LE] [kind u8] [seq u8] [inhoud]
//   0x4F10 audio:   [kanalen n] [crunch] [dB × 10, i16 LE] × n
//   0x4F11 kleuren: [types n] [dekking 0..255] [schermhelderheid × 100] [r g b] × n

const MSG_AUDIO = 0x4f10;
const MSG_COLORS = 0x4f11;
const LISTEN = "4F10-4F1F";
const KIND_SET = 1;
const KIND_STATE = 2;
const KIND_REQUEST = 3;

/** Mengpaneel: dB per kanaal (volgorde van het mengpaneel) en de bliksem-crunch */
export interface AudioState {
    mixDb: number[];
    crunch: number;
}

/** Kleurenpaneel: "#rrggbb" per deeltjestype, de dekking (stappen van 1/255) en de
 *  helderheid van het hele beeld (stappen van 0.01, 1 = zoals ontworpen) */
export interface ColorState {
    types: string[];
    opacity: number;
    screenBrightness: number;
}

export type LinkResult<T> = { ok: true; state: T } | { ok: false; message: string };

// ── Codering ───────────────────────────────────────────────────────────────

function header(msgType: number, kind: number, seq: number): number[] {
    return [msgType & 0xff, msgType >> 8, kind, seq];
}

function toHex(bytes: number[]): string {
    return bytes.map((b) => (b & 0xff).toString(16).padStart(2, "0")).join("");
}

function fromHex(hex: string): Uint8Array {
    const out = new Uint8Array(hex.length >> 1);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    return out;
}

function encodeAudio(seq: number, a: AudioState): number[] {
    const p = header(MSG_AUDIO, KIND_SET, seq);
    p.push(a.mixDb.length, a.crunch);
    for (const db of a.mixDb) {
        const v = Math.round(db * 10) & 0xffff;
        p.push(v & 0xff, v >> 8);
    }
    return p;
}

function decodeAudio(b: Uint8Array): AudioState | null {
    const n = b[4];
    if (b.length !== 6 + 2 * n) return null;
    const mixDb: number[] = [];
    for (let i = 0; i < n; i++) {
        const v = b[6 + 2 * i] | (b[7 + 2 * i] << 8);
        mixDb.push((v >= 0x8000 ? v - 0x10000 : v) / 10);
    }
    return { mixDb, crunch: b[5] };
}

function encodeColors(seq: number, c: ColorState): number[] {
    const p = header(MSG_COLORS, KIND_SET, seq);
    const clamp = (v: number, max: number) => Math.min(max, Math.max(0, v));
    p.push(c.types.length, Math.round(clamp(c.opacity, 1) * 255), Math.round(clamp(c.screenBrightness, 2) * 100));
    for (const hex of c.types) {
        p.push(parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16));
    }
    return p;
}

function decodeColors(b: Uint8Array): ColorState | null {
    const n = b[4];
    if (b.length !== 7 + 3 * n) return null;
    const types: string[] = [];
    for (let i = 0; i < n; i++) types.push("#" + toHex(Array.from(b.subarray(7 + 3 * i, 10 + 3 * i))));
    return { types, opacity: b[5] / 255, screenBrightness: b[6] / 100 };
}

// ── Verkeer ────────────────────────────────────────────────────────────────

const ERRORS: Record<string, string> = {
    no_master: "no OTA master connected (USB)",
    busy: "OTA master in use (flashing or traffic view in FlashDance)",
    uploading: "OTA master is flashing a module",
    old_firmware: "OTA master needs new firmware (espnow-kit/ota-master)",
    no_answer: "OTA master does not answer",
};

let bridge: Promise<boolean> | null = null;

/** Is de brug naar de OTA-master er (lokale dev-server)? */
export function bridgeAvailable(): Promise<boolean> {
    bridge ??= fetch("/espnow/status", { cache: "no-store" })
        .then((r) => (r.ok ? r.json() : null))
        .then((j) => j?.available === true)
        .catch(() => false);
    return bridge;
}

let seqCounter = Math.floor(Math.random() * 256);

async function transact<T>(
    msgType: number,
    packet: (seq: number) => number[],
    decode: (b: Uint8Array) => T | null,
): Promise<LinkResult<T>> {
    seqCounter = (seqCounter + 1) & 0xff;
    const seq = seqCounter;
    let reply: any;
    try {
        const r = await fetch("/espnow/exchange", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ packets: [toHex(packet(seq))], listen: LISTEN, wait: 2.5, expect: 1, repeat: 3 }),
        });
        reply = await r.json();
    } catch (e) {
        return { ok: false, message: `dev server unreachable (${e})` };
    }
    if (!reply.ok) return { ok: false, message: ERRORS[reply.error] ?? reply.message ?? "failed" };
    for (const { hex } of reply.received ?? []) {
        const b = fromHex(hex);
        if (b.length >= 4 && (b[0] | (b[1] << 8)) === msgType && b[2] === KIND_STATE && b[3] === seq) {
            const state = decode(b);
            if (state) return { ok: true, state };
        }
    }
    return { ok: false, message: "no reply from the installation (is the simulation running?)" };
}

export const installationAudio = {
    send: (a: AudioState) => transact(MSG_AUDIO, (seq) => encodeAudio(seq, a), decodeAudio),
    request: () => transact(MSG_AUDIO, (seq) => header(MSG_AUDIO, KIND_REQUEST, seq), decodeAudio),
};

export const installationColors = {
    send: (c: ColorState) => transact(MSG_COLORS, (seq) => encodeColors(seq, c), decodeColors),
    request: () => transact(MSG_COLORS, (seq) => header(MSG_COLORS, KIND_REQUEST, seq), decodeColors),
};

// ── Knoppenrij in een paneel ───────────────────────────────────────────────

export interface InstallationSync<T> {
    /** Huidige waarden op de pagina (in de precisie van het pakket) */
    current: () => T;
    /** Waarden van de installatie op de pagina zetten */
    apply: (state: T) => void;
    equal: (a: T, b: T) => boolean;
    send: (state: T) => Promise<LinkResult<T>>;
    request: () => Promise<LinkResult<T>>;
    /** Uitleg bij de knoppen */
    what: string;
}

/**
 * Voegt aan `container` een rij toe: "Installation  [Send] [Load]  status".
 * Alleen als de brug er is; dan vraagt hij meteen op wat de installatie nu heeft.
 * Geeft een functie terug die het paneel aanroept na elke wijziging.
 */
export function attachInstallationSync<T>(container: HTMLElement, sync: InstallationSync<T>): () => void {
    let changed = () => {};
    bridgeAvailable().then((available) => {
        if (!available) return;

        const row = document.createElement("div");
        row.className = "installation-sync";
        const label = document.createElement("span");
        label.textContent = "Installation";
        const send = document.createElement("button");
        send.textContent = "Send";
        send.title = `Send these ${sync.what} to the installation (saved there, also after a restart)`;
        const load = document.createElement("button");
        load.textContent = "Load";
        load.title = `Load the ${sync.what} of the installation into this page`;
        const status = document.createElement("span");
        status.className = "installation-status";
        row.append(label, send, load, status);
        container.appendChild(row);

        let installation: T | null = null;
        let busy = false;
        const setStatus = (text: string, tone: "" | "ok" | "warn" | "error" = "") => {
            status.textContent = text;
            status.dataset.tone = tone;
        };
        const refresh = () => {
            const dirty = installation !== null && !sync.equal(sync.current(), installation);
            send.classList.toggle("dirty", dirty);
            send.disabled = load.disabled = busy;
            if (busy || installation === null) return;
            setStatus(dirty ? "differs from this page" : "same as this page", dirty ? "warn" : "ok");
        };
        const run = async (
            pending: string,
            action: () => Promise<LinkResult<T>>,
            apply: boolean,
            done?: string,
        ) => {
            busy = true;
            refresh();
            setStatus(pending);
            const result = await action();
            busy = false;
            if (result.ok) {
                installation = result.state;
                if (apply) sync.apply(result.state);
                refresh();
                if (done && sync.equal(sync.current(), result.state)) setStatus(done, "ok");
            } else {
                refresh();
                setStatus(result.message, "error");
            }
        };

        send.addEventListener("click", () =>
            run("sending…", () => sync.send(sync.current()), false, "sent: the installation uses these now"),
        );
        load.addEventListener("click", () => run("loading…", sync.request, true, "loaded from the installation"));
        changed = refresh;
        run("checking…", sync.request, false);
    });
    return () => changed();
}
