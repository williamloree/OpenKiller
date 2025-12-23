# Test du Workflow CI/CD

## Comment tester le workflow

### 1. Vérifier que les fichiers sont prêts

Assurez-vous que tous les changements sont commités:

```bash
git status
```

### 2. Créer un commit de test avec conventional commit

```bash
# Pour tester une nouvelle fonctionnalité (MINOR bump)
git commit --allow-empty -m "feat: test CI/CD workflow"

# OU pour tester un bug fix (PATCH bump)
git commit --allow-empty -m "fix: test CI/CD workflow"

# OU pour tester un breaking change (MAJOR bump)
git commit --allow-empty -m "feat!: test CI/CD workflow"
```

### 3. Pousser sur main

```bash
git push origin main
```

### 4. Vérifier l'exécution

1. Allez sur GitHub → votre repo → onglet **Actions**
2. Vous devriez voir le workflow **"Build and Release"** en cours d'exécution
3. Cliquez dessus pour voir les logs en temps réel

### 5. Vérifier les résultats

Le workflow devrait:
- ✅ Détecter le type de version depuis le message de commit
- ✅ Bumper la version dans package.json
- ✅ Créer un tag git (ex: v1.0.2)
- ✅ Builder pour Windows, macOS et Linux
- ✅ Créer une release GitHub avec les installers

### Problèmes courants

#### Le workflow ne se déclenche pas
- Vérifiez que vous êtes sur la branche `main`
- Vérifiez que le commit ne contient pas `[skip ci]` dans le message

#### Erreur de permissions
- Allez dans Settings → Actions → General
- Sous "Workflow permissions", sélectionnez "Read and write permissions"
- Cliquez sur "Save"

#### Le build échoue
- Vérifiez les logs dans l'onglet Actions
- Assurez-vous que `npm ci` et `npm run build` fonctionnent localement

#### Les artifacts ne sont pas uploadés
- Vérifiez que les chemins dans le workflow correspondent aux fichiers générés
- Windows: `dist/*.exe`
- macOS: `dist/*.dmg`
- Linux: `dist/*.AppImage`

## Workflow actuel

Le workflow s'exécute automatiquement sur:
- ✅ Push direct sur `main`
- ✅ Pull Request mergée sur `main`

Il détecte automatiquement le type de version:
- `feat:` → version MINOR (1.0.0 → 1.1.0)
- `fix:` → version PATCH (1.0.0 → 1.0.1)
- `feat!` ou `BREAKING CHANGE` → version MAJOR (1.0.0 → 2.0.0)

## Désactiver temporairement

Pour désactiver le workflow sans le supprimer:
- Ajoutez `[skip ci]` au message de commit
- Exemple: `git commit -m "fix: update readme [skip ci]"`
