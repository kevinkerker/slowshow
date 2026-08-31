# Interne Notizen

Was hier liegt, ist Projektdokumentation für die Entwicklung — **nicht** für
Nutzer.

Der Ordner existiert getrennt von `docs/`, weil GitHub Pages `docs/` als
Website ausliefert und die Datenschutzerklärung von dort ihre öffentliche
Adresse bezieht (Play verlangt eine URL, keine Datei). Alles, was in `docs/`
liegt, wird damit zu einer indexierbaren Unterseite direkt neben ihr.

Für diese drei Dateien wäre das falsch:

| Datei | Warum nicht öffentlich |
| --- | --- |
| [store-listing.md](store-listing.md) | Entwurf des Store-Textes und der Abschnitt „Nicht geprüft" — im Repo ehrliche Dokumentation, als Unterseite neben der Datenschutzerklärung eine Einladung zum Missverstehen |
| [signing.md](signing.md) | beschreibt, wo der Signaturschlüssel liegt und wie er benutzt wird |
| [keystore.properties.example](keystore.properties.example) | Vorlage für eine Datei mit Passwörtern; enthält selbst keine, gehört aber nicht auf eine Website |

In `docs/` bleiben die drei Dokumente, die sich an Nutzer richten:
`privacy-policy.md`, `home-assistant.md` und `third-party-licenses.md`. Die
Lizenzübersicht gehört sogar ausdrücklich dorthin — RB-05 verlangt sie, und
Namensnennungspflichten erfüllt man nicht in einem privaten Ordner.

Wer hier eine Datei ergänzt, prüft bitte diese Frage: **Würde ich das einem
Fremden zeigen, der über eine Suchmaschine darauf stößt?** Wenn nein, gehört
es hierher.
