// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Klein laadscript voor de WordPress-embed. De embed-snippet laadt altijd
// particle-life-embed.js zonder versie, en de server (nginx) stuurt geen
// cache-instructies mee, dus Safari kon een oude bundel uit zijn cache halen
// naast een nieuwe WASM — die passen niet bij elkaar en de simulatie startte dan
// niet. Dit script verandert vrijwel nooit; het vraagt bij elk bezoek het
// actuele versienummer op (embed-version.json, door deploy.sh geschreven) en
// laadt daarmee de bijbehorende bundel en WASM. Zonder versiebestand (lokaal)
// valt het terug op een tijdstempel, dus altijd vers.

(() => {
    const scripts = Array.from(document.querySelectorAll<HTMLScriptElement>("script[src]"));
    const self = scripts.find((s) => /particle-life-embed\.js(\?|$)/.test(s.src));
    const base = self
        ? self.src.substring(0, self.src.lastIndexOf("/") + 1)
        : "https://michelreij.nl/webapps/origin-of-life/";

    const load = (version: string) => {
        (window as any).__OL_BUILD__ = version;
        const script = document.createElement("script");
        script.src = `${base}particle-life-embed-app.js?v=${encodeURIComponent(version)}`;
        document.head.appendChild(script);
    };

    fetch(`${base}embed-version.json?nc=${Date.now()}`, { cache: "no-store" })
        .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
        .then((j) => load(String(j.v)))
        .catch(() => load(String(Date.now())));
})();
