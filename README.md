# Open Killer

Application Electron moderne pour gérer les ports ouverts sur votre système.

## Structure du projet

```
OpenKiller/
├── src/
│   ├── main/           # Processus principal Electron
│   │   └── main.js     # Point d'entrée de l'application
│   ├── preload/        # Scripts de sécurité
│   │   └── preload.js  # Bridge sécurisé entre main et renderer
│   └── renderer/       # Interface utilisateur
│       ├── index.html  # Structure HTML
│       ├── renderer.js # Logique frontend
│       └── styles.css  # Styles CSS
├── assets/             # Ressources visuelles
│   └── icon.svg
├── docs/               # Documentation complète
│   ├── README.md       # Documentation détaillée
│   ├── QUICKSTART.md   # Guide de démarrage rapide
│   ├── PROJECT_OVERVIEW.md
│   ├── CONTRIBUTING.md
│   └── CHANGELOG.md
├── package.json        # Configuration du projet
└── LICENSE
```

## Démarrage rapide

```bash
# Installer les dépendances
npm install

# Lancer en mode développement
npm run dev

# Lancer normalement
npm start

# Compiler l'application
npm run build
```

## Documentation

Consultez le dossier [docs/](docs/) pour la documentation complète :
- [Guide de démarrage](docs/QUICKSTART.md)
- [Vue d'ensemble du projet](docs/PROJECT_OVERVIEW.md)
- [Guide de contribution](docs/CONTRIBUTING.md)
- [Historique des changements](docs/CHANGELOG.md)

## Licence

MIT - Voir [LICENSE](LICENSE) pour plus de détails.
