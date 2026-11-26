# OpenKiller – concept (v2, réécriture Rust)

> Écrit par A.Poc (`/projet`), validé par William le 2026-11-26. Source de vérité pour A.Dev et A.Github : ce qui n'est pas ici n'est pas à faire.
>
> **Cas particulier** : réécriture complète d'un projet existant (v1.2.0 Electron). Travail dans `D:\Open\OpenKiller` sur la branche `rust` (pas `D:\Projets`, pas `main`). Le code Electron (`src/`, `package.json`, `package-lock.json`, `node_modules/`, `dist/`, `docs/RELEASE.md`, `.github/workflows/build-release.yml`) est **supprimé** dans la v2 ; le comportement de référence est celui de la v1 (lire `src/main/main.js` et `src/renderer/renderer.js` avant de les supprimer).

## Identité

| Champ | Valeur |
| --- | --- |
| Nom | `openkiller` (affiché : OpenKiller) |
| Pitch | Trouve qui occupe un port et libère-le en un clic : un exe Windows natif de quelques Mo. |
| Type | `rust-bureau` |
| Agent de dev | `dev-rust-bureau` |
| Structure (MyLife) | freelance |
| Dossier | `D:\Open\OpenKiller` (branche `rust`) |
| Dépôt | `github.com/williamloree/OpenKiller` (public, MIT, existant) |
| Version de départ | **2.0.0** (succède à la v1.2.0 Electron ; `Cargo.toml` = référence de version) |
| Langue de l'interface | français |
| Plateforme | Windows 10/11 x64 uniquement (la v1 annonçait aussi macOS/Linux : abandonné) |

## Pourquoi

- **Problème** : un serveur de dev planté garde le port 3000/5173 ; il faut trouver le PID et le tuer. La v1 fait le travail mais c'est un Electron de ~150 Mo de RAM qui lance un PowerShell par PID à chaque rafraîchissement (lent, lourd).
- **Pour qui** : William (dev web au quotidien), et le public du dépôt.
- **Aujourd'hui sans l'outil** : `netstat -ano | findstr :3000` puis `taskkill /F /PID …`, ou la v1 Electron.
- **Projets similaires** : PortKiller (productdevbook, .NET sur Windows, pas de favoris ni RAM) ; Port Killer for Windows (WinForms C#, basique) ; portview / portwatch (TUI Rust, pas de fenêtre ni favoris) ; ClawPorts (Electron, lourd). Différence : exe natif minuscule, appels Win32 directs (aucun processus externe), favoris + bibliothèque, export, vidage RAM.

## MVP (version 2.0.0) – parité stricte avec la v1.2

| # | Fonctionnalité | C'est fait quand… |
| --- | --- | --- |
| 1 | **Liste des ports** : sockets TCP en écoute (`LISTEN`) et UDP liées, IPv4 + IPv6, via `GetExtendedTcpTable` / `GetExtendedUdpTable` (iphlpapi), sans `netstat` ni PowerShell. Une ligne par (protocole, port, PID) ; si IPv4 et IPv6 coexistent, une seule ligne (adresse IPv4 affichée). Colonnes : Port, Protocole, Adresse, Application, PID, RAM (Mo), Chemin. Rafraîchissement auto toutes les **10 s** + bouton Actualiser (F5), en conservant sélection, tri et défilement. | Lancer `python -m http.server 8123` → la ligne 8123 TCP / python.exe / PID / RAM / chemin apparaît en ≤ 10 s ; un scan complet prend < 50 ms (mesuré, journalisé en debug). |
| 2 | **Détails processus** : nom (sans `.exe`, comme la v1), mémoire (working set, Mo arrondi) via `GetProcessMemoryInfo`, chemin via `QueryFullProcessImageNameW`. Processus inaccessible (système, autre session) : nom depuis un instantané `CreateToolhelp32Snapshot`, RAM/chemin « N/A ». PID 0 = « Processus inactif du système », PID 4 = « System ». | Port 445 (System) s'affiche sans erreur avec « N/A » où il faut ; aucune ligne manquante faute de droits. |
| 3 | **Recherche, filtre, tri** : champ de recherche (port ou nom d'application, insensible à la casse, filtrage à la frappe), liste déroulante Protocole (Tous / TCP / UDP), tri par clic sur l'en-tête Port ou Application (asc/desc, flèche dans l'en-tête). Compteur « N ports » + état du filtre (« filtrés sur M »). | Taper `node` ne garde que les lignes node ; `5173` ne garde que ce port ; UDP masque le TCP ; double-clic sur l'en-tête inverse l'ordre. Fonctions pures testées. |
| 4 | **Tuer un processus** : bouton « Tuer » (et touche Suppr) sur la sélection. Sélection simple → confirmation (port, PID, application). Sélection multiple (cases à cocher de la ListView + Ctrl+A) → une confirmation qui liste les PID distincts. `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess` (forcé, comme `taskkill /F`). Refus explicite de tuer PID 0, 4 et le PID d'OpenKiller. Résultat dans la barre d'état (succès / « Accès refusé (processus admin ?) »), puis rafraîchissement. | Le `python -m http.server` ci-dessus est tué après confirmation et la ligne disparaît ; tuer un processus admin affiche « Accès refusé » sans planter ; la sélection multiple de 3 lignes de 2 PID tue 2 processus. |
| 5 | **Export** CSV ou JSON de la liste **affichée** (filtrée) via la boîte « Enregistrer sous » (`IFileSaveDialog`), nom proposé `openkiller-ports-AAAAMMJJ-HHMMSS.csv/.json`. CSV : mêmes en-têtes que la v1 (`Port,Protocole,Adresse,Application,PID,RAM (Mo),Chemin`), toutes cellules entre guillemets, `"` doublé, **cellule commençant par `= + - @` préfixée d'une apostrophe** (injection de formules), UTF-8 avec BOM (Excel). JSON : tableau d'objets `{port, protocol, address, processName, pid, memoryMB, exePath}` (mêmes clés que la v1, `null` si inconnu). Liste vide → message « Aucune donnée à exporter ». | Fichiers ouverts sans erreur dans Excel et par `ConvertFrom-Json` ; test unitaire d'échappement CSV (guillemets, virgules, retours à la ligne, `=cmd`). |
| 6 | **Favoris** : onglet « Favoris » (ListView : Port, Nom, Description, État ● Actif/Inactif, Application si actif). Ajouter / Modifier (dialogue : port 1-65535, nom obligatoire, description facultative ; port déjà favori refusé, comme la v1) / Supprimer (confirmation) / Voir (bascule sur Ports avec le port dans la recherche) / Tuer (si actif, même confirmation que n°4). **Bibliothèque** dans le dialogue d'ajout : les 12 presets de la v1 (3000 Nuxt / Node dev, 5173 Vite dev, 4200 Angular dev, 8000 Django dev, 5000 Flask dev, 80 Caddy / HTTP, 443 Caddy / HTTPS, 8080 HTTP alternatif, 5432 PostgreSQL, 3306 MySQL, 6379 Redis, 27017 MongoDB) ; un clic préremplit, ceux déjà en favoris sont grisés. Badges de compte sur les onglets (« Ports (N) », « Favoris (N) »). Stockage `%APPDATA%\openkiller\config.json`. Les favoris de la v1 (localStorage Electron) ne sont **pas** migrés. | Ajouter 5173 depuis la bibliothèque, le modifier, quitter, relancer → il est là ; lancer Vite → il passe « Actif » au rafraîchissement suivant ; validation testée (port 0, 70000, nom vide, doublon). |
| 7 | **Vider la RAM** : jauge en bas de la barre latérale (utilisée / totale en Go, part « libérable » = cache système, via `GlobalMemoryStatusEx` + `GetPerformanceInfo`), rafraîchie avec les ports. Bouton « Vider la RAM » → dialogue (Total, Utilisée, Libérable estimée) → relance `openkiller.exe clean-ram` **élevé** (`ShellExecuteExW` verbe `runas`, attente de fin) ; la sous-commande parcourt `EnumProcesses` et appelle `EmptyWorkingSet` sur chaque processus ouvrable (`PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION`), puis code de sortie 0. UAC refusé → « Opération annulée ». Après : barre d'état « RAM libérée : X Go » (différence avant/après). L'appli principale reste **non élevée**. | Clic → UAC → la jauge baisse ; UAC annulé → message, pas d'erreur ; `clean-ram` lancé sans droits admin fonctionne quand même sur les processus de l'utilisateur. |

**Transverse (obligatoire chaîne)** : mise à jour auto depuis les releases GitHub (`updater-windows.rs`, `REPO = "williamloree/OpenKiller"`, vérification 60 s après lancement puis toutes les 6 h, réglage `auto_update` dans la config) ; sous-commande `openkiller.exe update` ; entrée « Rechercher une mise à jour » (bouton/menu dans la barre latérale ou menu système de la fenêtre) ; `--demo` ; `--screenshot`. Instance unique (mutex) : relancer ramène la fenêtre existante.

**Raccourcis** : Ctrl+F (focus recherche), F5 (actualiser), Suppr (tuer la sélection), Ctrl+A (tout sélectionner dans la liste active), Échap (vider la recherche).

## Plus tard (pas dans le MVP)

- Icône dans la zone de notification (fermer = réduire, favoris actifs dans le menu).
- Lancement avec Windows.
- Import/export des favoris en JSON.
- Arrêt « doux » (WM_CLOSE puis forcé).
- Connexions établies (pas seulement en écoute).

## Hors périmètre

- macOS et Linux.
- Migration des favoris de la v1.
- Installeur NSIS / MSI : exe portable seul.
- Tuer des processus système protégés (pas d'élévation de l'appli principale).

## Interface

Fenêtre unique **redimensionnable** (la v1 était fixe 1200×800 ; taille mini 900×560, taille et position mémorisées dans la config), **contrôles natifs Win32 en thème sombre** :

- Barre de titre sombre (`DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)`), ListView / en-têtes / barres de défilement sombres (`SetWindowTheme(hwnd, "DarkMode_Explorer", …)` + couleurs ListView), fond `#1a1a1a` comme la v1, accent rouge de la v1 pour les actions de kill.
- **Barre latérale gauche** (≈ 200 px) : titre « OpenKiller », deux boutons-onglets « Ports (N) » / « Favoris (N) », bouton Actualiser, et en bas la **jauge RAM** dessinée en GDI (barre utilisée + segment libérable, libellé « 9,8 / 16,0 Go », « Libérable : 2,1 Go ») avec le bouton « Vider la RAM ».
- **Zone principale** : barre d'outils (recherche avec texte d'invite « Rechercher un port ou une application… », Protocole, Exporter CSV, Exporter JSON, Tuer la sélection) puis la ListView (mode Rapport, cases à cocher, sélection de ligne entière, info-bulle du chemin complet au survol de la colonne Chemin). État vide : « Aucun port trouvé ».
- Onglet Favoris : barre d'outils (Ajouter, Modifier, Supprimer, Voir, Tuer) + ListView ; état vide « Aucun favori – ajoute un port depuis la bibliothèque ».
- **Barre d'état** en bas (remplace les toasts de la v1) : dernier message + heure du dernier scan.
- Dialogues modaux natifs (confirmation de kill, ajout/modif de favori avec bibliothèque en liste, vider la RAM), sombres eux aussi.
- DPI per-monitor v2 (police Segoe UI à la bonne taille, mise en page en pixels mis à l'échelle). Navigation clavier complète (Tab, raccourcis), contrôles natifs = lecteur d'écran OK.

## Technique

- **Stack** : Rust stable (édition la plus récente), crate `windows` (dernière version) pour Win32/iphlpapi/psapi/DWM/Shell ; `serde` + `serde_json` (config, export JSON) ; `embed-resource` en build-dependency (icône + manifeste). Rien d'autre sans justification dans le commit.
- **Architecture proposée** :

| Fichier | Rôle |
| --- | --- |
| `Cargo.toml`, `build.rs`, `res/openkiller.rc`, `res/openkiller.manifest`, `res/openkiller.ico` | Exe unique, icône, manifeste (DPI v2, styles visuels, `asInvoker`) |
| `src/main.rs` | Arguments (`--demo`, `--screenshot <png>`, `update`, `clean-ram`), instance unique, lancement de l'UI |
| `src/ports.rs` | Tables TCP/UDP Win32 → `Vec<PortEntry>` ; dédoublonnage et tri purs (testés) |
| `src/process.rs` | Nom, chemin, working set d'un PID ; `kill(pid)` avec refus des PID protégés |
| `src/memory.rs` | État RAM (total, utilisée, libérable) ; `clean_ram()` (EmptyWorkingSet) ; relance élevée `runas` |
| `src/model.rs` | État pur de l'appli : filtre, recherche, tri, sélection, validation des favoris, presets (testé) |
| `src/export.rs` | CSV (échappement, anti-formules, BOM) et JSON (testé) |
| `src/config.rs` | `%APPDATA%\openkiller\config.json` (favoris, taille/position, `auto_update`), lecture tolérante |
| `src/demo.rs` | Données fictives pour `--demo` / `--screenshot` |
| `src/ui/*.rs` | Fenêtre principale, barre latérale, ListViews, dialogues, thème sombre, jauge RAM, capture |
| `src/updater.rs` | Copie du module de la chaîne (`~/.claude/projet/updater-windows.rs`) |

- **Données** : `%APPDATA%\openkiller\config.json` : `{ "favorites": [{ "port": 5173, "name": "Vite dev", "description": "" }], "window": { "x", "y", "w", "h" }, "auto_update": true }`. Écriture atomique (fichier temporaire + renommage). Exports : uniquement là où l'utilisateur choisit.
- **Réseau** : `api.github.com` (dernière release) et les hôtes de téléchargement des assets GitHub (`github.com` → `objects.githubusercontent.com` / `release-assets.githubusercontent.com`), HTTPS, uniquement pour la mise à jour. Rien d'autre.
- **Configuration** : aucune variable d'environnement ; réglages dans `config.json`.
- **Mise à jour** : auto depuis les releases GitHub (`openkiller.exe` + `openkiller.exe.sha256`). Les utilisateurs de la v1 Electron n'ont pas d'updater : ils téléchargent la v2 à la main (le README le dit).
- **CI** : remplacer `.github/workflows/build-release.yml` (montée de version auto Electron multi-OS) par le modèle `release-windows.yml` de la chaîne (A.Github). La release `v2.0.0` sera la première Rust ; les releases v1.x restent en place.

## Mode démo

`openkiller.exe --demo` : une quinzaine de ports fictifs réalistes (node 3000, vite/node 5173, python 8000, postgres 5432, redis-server 6379, mongod 27017, Caddy 80/443, Docker Desktop 2375, svchost 135, System 445, lsass 49664, spotify 57621 UDP, Code 9229…), 4 favoris (3000 actif, 5173 actif, 5432 actif, 4200 inactif), RAM 9,8 / 16,0 Go dont 2,1 libérable. Kill et Vider la RAM **simulés** (la ligne disparaît, la jauge baisse, message « (démo) »). **Rien n'est écrit** : ni config, ni export réel (la boîte d'export s'ouvre mais l'écriture est simulée), aucun appel réseau, pas d'updater.

`openkiller.exe --screenshot <fichier.png> [--view ports|favorites|kill|presets]` : ouvre en démo, affiche la vue demandée (liste des ports, favoris, dialogue de confirmation de kill multiple, dialogue d'ajout avec bibliothèque), capture la fenêtre (avec le dialogue au premier plan) via `PrintWindow`/BitBlt, écrit le PNG et quitte. Encodage PNG via WIC (Win32, pas de crate).

## README prévu

- **Captures** (dans `assets/screenshots/`, produites par `--screenshot`) : `ports.png` (liste des ports, jauge RAM), `favoris.png`, `kill.png` (confirmation multiple), `bibliotheque.png` (dialogue d'ajout avec presets).
- **Mesures** : comparatif **v1.2.0 Electron vs v2.0.0 Rust** : taille téléchargée (installeur v1 vs exe v2), RAM au repos (somme des processus Electron vs exe), temps jusqu'à la fenêtre affichée, durée d'un scan des ports (v1 : `netstat` + PowerShell par PID ; v2 : appels Win32), CPU au repos. Méthode écrite dans le README.
- **Sections propres** : « Ce que tu vois » (barre latérale, liste, favoris, jauge), raccourcis clavier, « Venir de la v1 » (favoris non migrés, plus de macOS/Linux, désinstaller l'ancienne version).
- Remplacer `assets/icon.svg` seulement si besoin ; garder le logo existant pour le README et l'icône `.ico`.

## Sécurité

- L'appli **tue des processus** : refus des PID 0, 4 et soi-même ; confirmation obligatoire ; PID re-vérifié au moment du kill (le processus visé a toujours le même nom ; sinon « Le processus a changé, actualise »), contre la réutilisation de PID entre affichage et clic.
- **Élévation** : seule la sous-commande `clean-ram` tourne en admin ; elle n'accepte aucun autre argument et ne lit aucune entrée ; chemin relancé = `std::env::current_exe()` (pas de chemin construit depuis l'extérieur). L'appli principale est `asInvoker`.
- **Entrées non fiables** : noms et chemins de processus (affichés et exportés) → échappement CSV + anti-formules, JSON via serde ; `config.json` modifiable à la main → lecture tolérante, ports hors 1-65535 et doublons ignorés, longueurs bornées (nom 100, description 500 caractères).
- Mise à jour : HTTPS, SHA-256 vérifié avant remplacement (module de la chaîne).
- Aucun secret.

## Risques et inconnues

| Risque | Parade |
| --- | --- |
| Mode sombre des contrôles natifs (API non documentées `SetPreferredAppMode`/`AllowDarkModeForWindow` d'uxtheme) | Utiliser seulement le documenté : `DwmSetWindowAttribute` + `SetWindowTheme("DarkMode_Explorer")` + `WM_CTLCOLOR*` + couleurs ListView ; boutons/listes déroulantes non sombres acceptés s'il le faut (noter l'écart). |
| UDP n'a pas d'état « écoute » : beaucoup de sockets UDP éphémères | Afficher toutes les UDP liées (comme `netstat -a`) ; le filtre Protocole permet de les masquer. Noter le nombre en mode démo. |
| Réutilisation de PID entre scan et kill | Re-vérification du nom au moment du kill (voir Sécurité). |
| EmptyWorkingSet provoque des défauts de page ensuite (gain ressenti discutable) | Même comportement que la v1 ; texte du dialogue honnête (« libère la mémoire physique que les applications ne réutilisent pas tout de suite »). |
| Captures d'un dialogue modal par la même appli | `--screenshot` compose fenêtre + dialogue (PrintWindow de chacun) ; sinon capture de l'écran sur la zone de la fenêtre. |
| L'agent de dev doit travailler hors de `D:\Projets` et sur la branche `rust` | Dit explicitement dans la consigne ; pas de push. |

## Critères de « fini » pour la 2.0.0

- [ ] Toutes les lignes du MVP passent leur critère.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release` verts en local et en CI.
- [ ] Mode démo OK, 4 captures produites par `--screenshot`.
- [ ] Code Electron supprimé, `.gitignore` à jour (`target/`).
- [ ] Revue sécurité passée.
- [ ] README réécrit (comparatif v1/v2), workflow de release Windows, release `v2.0.0` publiée avec `openkiller.exe` + `.sha256`.
