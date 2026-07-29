# Open Killer

Application Electron pour visualiser et gérer les ports ouverts sur votre système : repérez un port occupé, identifiez le processus qui l'utilise, et terminez-le en un clic.

## Fonctionnalités

**Ports ouverts**
- Liste en temps réel des ports en écoute (TCP/UDP), rafraîchie automatiquement toutes les 10 secondes
- Pour chaque port : protocole, adresse, application, PID, mémoire utilisée (RAM) et chemin de l'exécutable (au survol)
- Recherche par port ou nom d'application, filtre par protocole
- Tri par port ou par application
- Terminaison d'un processus (Kill) avec confirmation
- Sélection multiple pour terminer plusieurs processus en une seule action
- Export de la liste affichée en CSV ou JSON

**Favoris**
- Épingler des ports pour les retrouver rapidement, avec nom et description
- Bibliothèque de ports courants (Nuxt, Vite, Caddy, PostgreSQL, MySQL, Redis, MongoDB...) pour un ajout en un clic
- Modification et suppression d'un favori
- Indicateur visuel si le port favori est actuellement actif

Compatible Windows, macOS et Linux.

## Démarrage rapide

```bash
# Installer les dépendances
npm install

# Lancer en mode développement (DevTools ouverts)
npm run dev

# Lancer normalement
npm start

# Compiler l'application (installeur)
npm run build
```

## Structure du projet

```txt
OpenKiller/
├── src/
│   ├── main/           # Processus principal Electron
│   │   └── main.js     # Fenêtre, scan des ports, gestion des processus (IPC)
│   ├── preload/        # Bridge sécurisé (contextIsolation)
│   │   └── preload.js
│   └── renderer/       # Interface utilisateur
│       ├── index.html
│       ├── renderer.js
│       └── styles.css
├── assets/              # Ressources visuelles
├── docs/                # Documentation complémentaire
├── package.json
└── LICENSE
```

## Stack technique

- [Electron](https://www.electronjs.org/) — fenêtre native, IPC sécurisé (`contextIsolation`, pas de `nodeIntegration` côté renderer)
- JavaScript vanilla côté interface (pas de framework)
- Commandes système natives pour le scan des ports et processus (`netstat`/PowerShell sur Windows, `lsof`/`ps` sur macOS et Linux)

## Licence

MIT - voir [LICENSE](LICENSE) pour plus de détails.
