# CoopaCrypt — Analyse et feuille de route

> État des lieux réalisé le 2026-09-02 sur la branche `master` (commit `dfaefce`).
> Les décisions de la section 5 sont arbitrées ; le détail d'exécution reste à affiner.

---

# Partie I — État des lieux

## 1. Ce que fait le projet aujourd'hui

Éditeur de texte WPF (.NET 8, Windows uniquement) qui ouvre et enregistre des fichiers
`.coocrypt` chiffrés en AES. Deux projets dans la solution :

- `CoopaCrypt` — l'application WPF
- `WapProj` — packaging MSIX, publié en release GitHub via GitHub Actions

### Cartographie du code

| Fichier | Rôle |
|---|---|
| `CoopaCrypt/MainWindow.xaml.cs` | Éditeur `TextBox`, menu New/Open/Save, Find/Replace, raccourcis clavier, ouverture par argument de ligne de commande |
| `CoopaCrypt/Crypto.cs` | AES-CBC/PKCS7, chiffre/déchiffre tout le fichier d'un bloc |
| `CoopaCrypt/Pops/PopCrypto.xaml.cs` | Boîte de saisie du mot de passe |
| `CoopaCrypt/Pops/PopFind.xaml.cs` | Rechercher / Remplacer |
| `CoopaCrypt/FileAssociation.cs` | Enregistre l'extension `.coocrypt` dans HKCU au démarrage |
| `CoopaCrypt/WindowExtension.cs` | Persiste position / taille / opacité des fenêtres en JSON |
| `CoopaCrypt/Extensions.cs` | Helpers JSON + `HashString` (SHA256) |

### Flux fonctionnel

- **Save** → `SaveFileDialog` → popup mot de passe → `Crypto.Crypt` écrit les octets chiffrés
- **Open** → popup mot de passe → `Crypto.Decrypt` → remplit la `TextBox`

## 2. Le point structurant : le schéma cryptographique

Maillon faible actuel (`CoopaCrypt/Crypto.cs`, lignes 15-33).

### 2.1 Clé = `SHA256(mot de passe)`, sans sel ni KDF

Un SHA256 simple se teste à des milliards de candidats par seconde sur GPU. Ni sel
(pas de protection contre les tables précalculées), ni facteur de coût.

**Correctif :** Argon2id (ou PBKDF2 à itérations élevées), avec sel aléatoire par fichier.

### 2.2 IV constant et public

L'IV est dérivé du hash de `Assets/chiffrement.png` embarqué dans l'application, puis
tronqué en `IV[8..24]`. Il est donc **identique pour tous les fichiers de tous les
utilisateurs**. Deux fichiers chiffrés avec le même mot de passe et un même début de
contenu produisent des blocs identiques : fuite d'information.

**Correctif :** nonce aléatoire par fichier, stocké en clair en tête (c'est son usage
normal, il n'a pas à être secret).

### 2.3 Aucune authentification du chiffré

CBC sans HMAC : contenu modifiable par un tiers sans détection, et exposition aux
attaques par oracle de padding.

**Correctif :** chiffrement authentifié (AEAD).

### 2.4 Aucune détection de mauvais mot de passe

Un mot de passe erroné produit soit une exception de padding, soit du charabia
silencieux. L'utilisateur ne peut pas distinguer les deux cas.

**Correctif :** le tag d'authentification AEAD fournit nativement cette détection.

### 2.5 Pas d'en-tête de format

Ni magic bytes, ni numéro de version, ni sel. **Toute évolution du chiffrement casse
les fichiers existants**, sauf chemin de lecture legacy explicite.

## 3. Autres constats

### 3.1 Fonctionnel / UX

- **Pas de notion de « fichier courant »** : Save redemande à chaque fois le chemin *et*
  le mot de passe. Pas de Ctrl+S rapide ni de « Save As » distinct.
- **Pas de détection de modifications non enregistrées** : fermer la fenêtre perd le
  travail sans avertissement.
- **Mauvaise boîte de dialogue** : `SaveAction_Click` utilise un `OpenFileDialog` pour
  enregistrer (`MainWindow.xaml.cs:78`).

### 3.2 Bugs identifiés

- **`FindNext`** passe le texte cherché à `Regex.Matches` sans échappement : chercher
  `(` ou `[` lève une exception non gérée.
- **`ReplaceAll`** fait un `Rtb.Text.Replace(...)` sur les valeurs distinctes : ignore
  l'option « sensible à la casse » et remplace au-delà des correspondances réelles.
- **Emplacement de la configuration** : les `*-config.json` sont écrits à côté de
  l'exécutable (`WindowExtension.cs:11`), dossier en lecture seule en MSIX ou sous
  Program Files. `%APPDATA%` serait le bon emplacement.

### 3.3 Projet / CI

- `Newtonsoft.Json` référencé mais jamais utilisé (le code utilise `System.Text.Json`).
- `Extensions.ToBytes` / `FromBytes` : code mort.
- **Aucun test** : `dotnet test` est commenté dans le CI.
- Le job CI ne définit pas `Solution_Path` (seulement `Solution_Name`) : le `msbuild` ne
  fonctionne que parce qu'il trouve la solution dans le dossier courant.
- Le CD se déclenche sur `pull_request: closed` **sans vérifier**
  `github.event.pull_request.merged == true` : fermer une PR sans la merger publie une
  release.
- `AppxPackageSigningEnabled: False` dans le CD : la signature est explicitement
  désactivée, indépendamment de la question du certificat.
- `WapProj/WapProj_TemporaryKey.pfx` est versionné dans le dépôt.

---

# Partie II — Axes d'évolution souhaités

1. **Support du markdown** dans l'éditeur : preview et édition ligne par ligne.
2. **Support multi-OS** : Linux (glibc et musl), macOS, Windows, et potentiellement
   Android / iOS. ~~Extension Nextcloud~~ *(abandonné)*.
3. **Releases signées** sur GitHub : aujourd'hui l'exécutable n'est pas signé, l'install
   affiche « Éditeur inconnu » et impose de passer par « Informations complémentaires ».
4. **Qualité du chiffrement** (cf. section 2).
5. **Volet de navigation** : des « onglets » dans le document pour mieux organiser
   l'information.

---

# Partie III — Décisions

## 4. Contexte arbitré

| Question | Réponse |
|---|---|
| Extension Nextcloud | **Abandonnée** |
| Langages accessibles | TypeScript / web, Rust, ouvert à l'apprentissage |
| Ambition | **Produit open source diffusé** (releases publiques, signature, doc, contributions) |

## 5. Décision de stack : cœur Rust + shell Tauri v2 + CodeMirror 6

### Le raisonnement

Le facteur décisif est l'**axe 1 combiné à l'axe 5**. Un éditeur markdown avec preview
live et panneau de navigation représente l'essentiel du travail d'UI du projet :

- **En web** : CodeMirror 6 ou Milkdown, écosystème mature, quelques jours d'intégration.
- **En Avalonia / WPF** : aucun contrôle markdown sérieux n'existe. Il faut construire
  l'éditeur, la coloration syntaxique, la preview synchronisée et l'outline à la main.
  Plusieurs mois de plomberie qui n'apportent rien de différenciant au produit.

Facteurs secondaires qui vont dans le même sens :

- **Linux musl** : trivial en Rust, peu balisé en .NET + Avalonia.
- **Taille des binaires** : déterminante pour une diffusion open source.
- **Mobile** : Tauri v2 supporte Android/iOS ; Avalonia y arrive mais son support mobile
  est plus jeune.
- **Compétences** : Rust et TypeScript sont accessibles, la contrainte C# ne s'applique pas.

### Le coût assumé

C'est une réécriture, et elle est franche : **aucune compatibilité avec l'ancien format
n'est maintenue.** Il n'existait qu'un seul fichier en production et il n'a pas à être
repris, ce qui dispense de porter indéfiniment un chemin de déchiffrement cassé par
conception. L'application .NET reste dans le dépôt jusqu'à la parité fonctionnelle, puis
sera archivée.

### Alternative écartée

**.NET + Avalonia** : capitalise sur le C# existant et couvre desktop + mobile. Écarté
uniquement à cause du coût de construction de l'éditeur markdown, pas pour des raisons
de portabilité.

## 6. Principe transverse : spécifier le format avant de coder

Un document `FORMAT.md` décrivant le format de fichier v2 — magic bytes, numéro de
version, paramètres Argon2id, sel, nonce, tag d'authentification, ordre des octets —
accompagné de **vecteurs de test** (mot de passe + clair → chiffré attendu).

Ce travail ne dépend d'aucun langage, survit à la décision de réécriture, et devient le
contrat de conformité de toute implémentation future. C'est le premier livrable.

## 7. Axes 1 et 5 : un seul et même sujet

Les titres markdown (`#`, `##`) fournissent l'arborescence gratuitement : le panneau
latéral se construit en parsant les titres, pas en inventant un format d'onglets
propriétaire. Un `#` de niveau 1 = un chapitre.

**Le volet est un navigateur, pas un sommaire.** L'éditeur affiche **un seul chapitre à
la fois** ; les chapitres ne défilent pas les uns derrière les autres. Objectif : confort
de lecture.

**Le découpage est une vue, pas un format de stockage.** Le fichier reste un document
markdown continu ; l'application le découpe à l'affichage. Le format de fichier n'est
donc pas impacté, et le document reste lisible par n'importe quel outil une fois
déchiffré — on conserve le principe « c'est du texte ».

Corollaire non négociable : **la recherche porte sur le document entier**, chapitres
masqués inclus, et un résultat fait basculer vers son chapitre.

**→ Spécification complète : [`UI.md`](UI.md)**

## 8. Axe 3 : la signature, plateforme par plateforme

Coûts indépendants du choix de stack.

| Plateforme | Solution | Coût |
|---|---|---|
| **Windows** | SignPath Foundation (gratuit pour l'open source) — *à confirmer auprès d'eux* | 0 € |
| | Repli : Azure Trusted Signing (exige une entité légale de 3 ans d'ancienneté) | ~10 $/mois |
| | Repli : certificat OV classique (token matériel ou HSM cloud obligatoire) | 300-400 €/an |
| **macOS** | Apple Developer Program + notarisation (obligatoire, sinon Gatekeeper bloque) | 99 $/an |
| **iOS** | Couvert par le même compte Apple | — |
| **Android** | Clé auto-signée hors store ; Play Store : 25 $ une fois | 0-25 $ |
| **Linux** | Pas de signature au sens Windows ; Flatpak / AppImage avec signature GPG | 0 € |

---

# Partie IV — Feuille de route

## Phase 0 — Spécification du format *(aucun code)* — **terminée**

- [x] Rédiger [`FORMAT.md`](FORMAT.md) : format v2 complet et non ambigu
- [x] Choisir l'AEAD → XChaCha20-Poly1305 (nonce de 192 bits tirable aléatoirement)
- [x] Fixer les paramètres Argon2id → 128 MiB, t=4, p=4
- [x] Trancher la normalisation du mot de passe → NFC (RFC 8265)
- [x] Trancher le changement de mot de passe → re-chiffrement complet
- [x] Trancher le durcissement des paramètres → automatique à chaque enregistrement
- [ ] ~~Produire les vecteurs de test~~ → **reporté en phase 1** : ils seront générés par
      l'implémentation de référence, puis gelés. Aucune valeur ne doit être inventée.

## Phase 1 — Cœur Rust (`coopacrypt-core`) — **faite**

- [x] Implémentation du format v2
- [x] **Vecteurs de test produits et gelés** — 13 vecteurs dans
      `crates/coopacrypt-core/tests/vectors.json`
- [x] Effacement mémoire des secrets (`zeroize`)
- [x] Suite de tests : 41 tests — aller-retour, conformité aux vecteurs, cas d'erreur
- [x] `#![forbid(unsafe_code)]`, clippy propre, aucun avertissement
- [x] `inspect()` : lecture de l'en-tête sans mot de passe
- [x] ~~Support du format v1~~ → **abandonné.** Aucune migration à préparer, le module
      `legacy` et ses trois dépendances (`aes`, `cbc`, `sha2`) ont été retirés. Un chemin
      de déchiffrement cassé en moins dans le code.
- [ ] ~~Sauvegarde atomique~~ → **déplacée en phase 2** : le crate ne fait aucune
      entrée-sortie disque, ce qui le garde testable et portable sans condition.

## Phase 2 — CLI `coopacrypt` — **faite**

Crate `crates/coopacrypt-cli`. Banc d'essai du format et premier livrable utilisable,
notamment sur Linux et Linux musl où aucune interface graphique n'est nécessaire.

- [x] `new` — crée un coffre vide
- [x] `encrypt <src> -o <dst>` — chiffre un fichier texte (ou l'entrée standard via `-`)
- [x] `decrypt <coffre> [-o <dst>]` — sortie standard par défaut
- [x] `info <coffre>` — en-tête, **sans mot de passe**
- [x] `chpass <coffre>` — re-chiffrement complet
- [x] Écriture atomique (temporaire + `sync_all` + remplacement), avec nettoyage du
      temporaire si le remplacement échoue
- [x] Refus d'écraser un fichier existant : un coffre n'a pas de sauvegarde
- [x] Rejet immédiat d'un fichier sans le magic, **avant** toute saisie de mot de passe
- [x] Confirmation du mot de passe à la création, avertissement si la phrase est courte
- [x] Saisie masquée sur terminal, lecture sur l'entrée standard si elle est redirigée —
      ce qui rend la CLI scriptable et testable en intégration continue

### Tests d'intégration

`crates/coopacrypt-cli/tests/cli.rs` — la CLI est lancée comme un vrai processus.

| Scénario | Vérifie |
|---|---|
| Aller-retour `encrypt` → `decrypt` | Contenu identique à l'octet près (accents, emoji, CRLF, absence de fin de ligne finale) |
| Mauvais mot de passe | Rejet, code retour non nul |
| Un octet du chiffré modifié | Rejet — le tag fait son travail |
| `chpass` puis relecture | Nouveau mot de passe accepté, ancien refusé |
| Deux contenus de tailles très différentes | Fichiers de taille identique : le remplissage masque bien la longueur |
| Écrasement d'un coffre existant | Refusé |
| `info` sur un fichier quelconque | Refusé sans demander de mot de passe |
| Confirmation divergente | Refusé, et aucun fichier créé |
| Mot de passe vide | Refusé |

Les quatre premiers déclenchent des dérivations Argon2id réelles : ils sont marqués
`#[ignore]` pour ne pas alourdir un `cargo test` ordinaire.

```text
cargo test                          # 40 tests, ~1,5 s
cargo test --release -- --ignored   # 4 tests d'intégration, ~9 s
```

**Aucun réglage permettant d'affaiblir le KDF n'est exposé par le binaire**, même pour
les tests : un outil de production n'a pas à embarquer un bouton « chiffrer moins bien ».

## Phase 3 — Application desktop (Tauri v2) — cf. [`UI.md`](UI.md)

Deux morceaux : `crates/coopacrypt-app` (backend Rust) et `app/` (front TypeScript,
Vite + CodeMirror 6).

- [x] Shell Tauri, cycle de vie fichier (nouveau, ouvrir, Ctrl+S, enregistrer sous,
      changer de mot de passe)
- [x] Analyseur de structure : chapitres sur `#`, sous-parties sur `##`+ — **via l'arbre
      syntaxique Lezer**, pas une expression régulière, ce qui règle nativement le cas du
      `#` dans un bloc de code clôturé
- [x] Éditeur CodeMirror 6 avec markdown et coloration, limité au chapitre courant
- [x] Volet de navigation : arborescence, chapitre courant marqué, position de défilement
      mémorisée par chapitre
- [x] Prévisualisation synchronisée, limitée au chapitre affiché, **désinfectée**
- [x] Édition par tranche : la modification s'applique au document complet, état
      « modifié » global, scission et fusion de chapitres suivies sans perte du curseur
- [x] Recherche sur le **document entier**, résultats groupés par chapitre avec extraits,
      `F3` basculant de chapitre
- [x] Remplacer tout : annonce occurrences **et** chapitres touchés
- [x] Bugs de recherche de la v1 corrigés : terme traité en texte littéral, casse
      réellement respectée
- [x] Verrouillage automatique après inactivité (10 min), échéance appliquée **côté
      Rust** ; au verrouillage, volet et prévisualisation se vident comme l'éditeur
- [x] Le mot de passe ne réside jamais dans le contexte JavaScript
- [x] Titre de fenêtre constant, jamais dérivé du contenu

### Deux points de sécurité qui ont dicté la conception

**Le mot de passe vit côté Rust.** Chaque enregistrement tire un nouveau sel et exige
donc une nouvelle dérivation : il faut bien conserver le mot de passe pendant la session.
Le garder dans `Session` (effacé par `zeroize`) plutôt que dans le contexte JavaScript le
met hors de portée du DOM, des outils de développement et de toute dépendance front. La
page ne l'envoie qu'au déverrouillage et ne le revoit jamais. Le contenu déchiffré, lui,
réside forcément dans la page — c'est ce que l'éditeur affiche ; cette asymétrie est
assumée.

**La prévisualisation est désinfectée, et ce n'est pas optionnel.** Le markdown est rendu
en HTML *dans la vue Tauri*, laquelle a accès au pont IPC : un `<script>` ou un `onerror=`
glissé dans un document s'exécuterait avec les droits de l'application et pourrait appeler
`vault_save` ou exfiltrer le contenu. Le coffre vient de l'utilisateur, mais il se copie
et se synchronise — rien ne garantit qu'il soit resté sous son seul contrôle. D'où
DOMPurify, une politique de sécurité de contenu stricte dans `tauri.conf.json`, et des
liens neutralisés dans la prévisualisation (une requête réseau trahirait le contenu, ne
serait-ce que par l'URL appelée).

### Tests

```text
cargo test -p coopacrypt-app     # 6 tests : session, délai d'inactivité, écriture atomique
npm --prefix app test            # 33 tests : découpage en chapitres, recherche
npx tsc --noEmit                 # typage strict
```

Le découpage et la recherche sont testés à part de l'interface : ce sont eux qui portent
la logique, et ils n'ont besoin d'aucun DOM.

### Limitation connue

**L'annulation ne traverse pas les chapitres.** `UI.md` §3 demande un historique global ;
aujourd'hui, changer de chapitre entre dans l'historique de CodeMirror, si bien qu'annuler
juste après une bascule restaure la tranche précédente au lieu de défaire la dernière
vraie édition. Le corriger suppose de tenir la pile d'annulation au niveau du document et
non de l'éditeur — un chantier à part entière. Le défaut est documenté dans
`app/src/editor.ts` plutôt que masqué.

### Reste à faire

- [ ] Historique d'annulation global traversant les chapitres
- [ ] Configuration persistante (taille de fenêtre, dernier chapitre ouvert) dans le
      répertoire utilisateur standard, **sans aucune donnée dérivée du contenu** — index
      de chapitre, jamais son titre
- [ ] Réorganisation des chapitres par glisser-déposer (`UI.md` §7)
- [ ] Tests de bout en bout de l'interface

## Phase 4 — Distribution

### Intégration continue — **faite**

`.github/workflows/ci.yml`, en remplacement du workflow .NET.

- [x] Qualité : `cargo fmt`, `clippy` (avertissements en erreurs), tests Rust, tests
      d'intégration de la CLI, typage TypeScript, tests du front
- [x] Compilation vérifiée sur Linux, Windows et macOS
- [x] **Garde-fou des vecteurs de test** : le workflow régénère `vectors.json` et
      échoue s'il diffère. Un changement de format ou de paramètres par défaut ne peut
      plus passer inaperçu.

### Release — **faite**

`.github/workflows/release.yml`, déclenchée par une étiquette `v*`.

L'ancien CD se déclenchait sur `pull_request: closed` **sans vérifier** `merged == true` :
fermer une pull request sans la fusionner publiait une release. Le déclencheur par
étiquette supprime le problème et porte le numéro de version.

| Cible | Processeurs | Paquets |
|---|---|---|
| Windows | x64, ARM64 | MSI + NSIS ; NSIS seul en ARM64, WiX ne produisant pas de MSI ARM64 |
| macOS | Apple Silicon, Intel | `.dmg` par architecture |
| Linux | x64, ARM64 | `.deb`, `.rpm`, `.AppImage` |
| CLI, toutes distributions | x64, ARM64 | binaire **statique musl** |
| CLI, Windows et macOS | x64, ARM64 | archives |

- [x] Vérification que l'étiquette correspond à la version de `tauri.conf.json`
- [x] Release créée en brouillon, publiée seulement une fois tous les paquets déposés
- [x] `SHA256SUMS` généré — seul moyen de vérification en l'absence de signature
- [x] Contrôle que le binaire musl est **réellement statique** (`ldd`), sans quoi la
      cible perdrait tout intérêt
- [x] Recette Arch (`packaging/PKGBUILD`) et flake Nix (`flake.nix`), que Tauri ne
      produit pas

### Précision sur musl

**La CLI est statique, l'application graphique ne peut pas l'être.** Son interface est
une vue web native, liée à webkit2gtk et donc à la glibc. Il n'existe pas de build musl
statique de l'interface, et prétendre le contraire serait malhonnête. Les distributions
sans `.deb` ni `.rpm` passent par l'AppImage, la recette Arch ou le flake Nix.

### Reste à faire

- [ ] Signature Windows (SignPath Foundation, gratuit pour l'open source) et
      notarisation macOS
- [ ] Retirer `WapProj_TemporaryKey.pfx` de l'historique Git
- [ ] Documentation utilisateur
- [ ] **Vérifier les workflows sur une vraie exécution** : ils n'ont pu être validés que
      syntaxiquement. Les points les plus incertains sont les runners ARM
      (`ubuntu-22.04-arm`, `windows-11-arm`, réservés aux dépôts publics) et la
      compilation du front sur Windows ARM64.
- [ ] Faire construire le flake Nix et la recette Arch par quelqu'un qui dispose de ces
      systèmes : ni Nix ni pacman n'étaient disponibles à leur rédaction.

## Phase 5 — Mobile

- [ ] Android
- [ ] iOS

## Phase 6 — Archivage de l'application .NET

- [ ] Archiver l'application WPF une fois la parité fonctionnelle atteinte

---

## 9. Arbitrages de la phase 0

| Question | Décision |
|---|---|
| Coffre mono-fichier ou synchronisé ? | **Mono-fichier autonome.** L'utilisateur doit pouvoir déplacer son fichier sur une clé USB ou un Nextcloud sans perte de sécurité. **Aucune infrastructure attaquable.** |
| Fichiers `.coocrypt` en production ? | **Un seul, et il n'est pas repris.** Aucune compatibilité descendante à maintenir. |
| Conserver l'extension `.coocrypt` ? | **Oui.** Les versions se distinguent par les magic bytes, pas par l'extension. |
| Niveau de chiffrement visé ? | **Le plus fort possible**, la performance étant indifférente sur des fichiers de cette taille. |

### 9.1 Ce que « le plus fort possible » signifie ici

Le plafond de sécurité d'un coffre protégé par mot de passe est fixé par **le KDF et
l'entropie du mot de passe**, pas par le choix du chiffre. AES-256-GCM et
XChaCha20-Poly1305 offrent tous deux ~128 bits de sécurité effective contre toutes les
attaques connues ; aucun n'est « plus fort » que l'autre.

Le budget se dépense donc sur Argon2id — et c'est là, contrairement au chiffre, que la
performance devient perceptible (~0,25 s par ouverture à 128 MiB).

**Le chiffrement en cascade a été écarté.** Superposer deux AEAD couvre un risque
théorique quasi nul au prix d'un doublement de la surface de code cryptographique, donc
du risque réel : l'erreur d'implémentation.

### 9.2 Conséquences du mono-fichier portable sur le format

1. **En-tête auto-descriptif** : tous les paramètres KDF vivent dans le fichier. Il
   s'ouvre sur n'importe quelle machine, avec n'importe quelle version, et les paramètres
   peuvent être durcis plus tard sans casser l'existant.
2. **Aucune dépendance à un trousseau système** : ce serait rompre la portabilité. Une
   éventuelle intégration Windows Hello / Touch ID ne sera qu'un cache de confort, jamais
   la source de vérité.
3. **Remplissage à 4 Kio** : un hébergeur de synchronisation voit la taille du fichier et
   son évolution. Le remplissage masque la longueur exacte du contenu.
4. **Écriture atomique obligatoire** : un dossier synchronisé peut être lu pendant
   l'écriture.

### 9.3 Décisions techniques retenues

| Élément | Choix |
|---|---|
| KDF | Argon2id — 128 MiB, 4 itérations, parallélisme 4, sortie 32 octets, sel 16 octets |
| AEAD | XChaCha20-Poly1305 — nonce 192 bits tiré aléatoirement sans risque de collision |
| Normalisation du mot de passe | **NFC** (RFC 8265, profil PRECIS `OpaqueString`) puis UTF-8 |
| Remplissage | Multiple de 4096 octets |
| En-tête | 64 octets en clair, authentifiés intégralement comme AAD |
| Changement de mot de passe | Re-chiffrement complet du fichier |
| Durcissement des paramètres | Automatique et silencieux à chaque enregistrement |
| Compatibilité v1 | **Aucune.** Le format v1 n'est ni lu ni documenté ; un fichier sans le magic `COOCRYPT` est rejeté. |

**→ Spécification complète : [`FORMAT.md`](FORMAT.md)**

### 9.4 Normalisation du mot de passe : NFC, pas NFKC

UTF-8 est un **encodage** ; la normalisation Unicode est une question distincte. Un même
caractère admet plusieurs écritures valides : « é » s'écrit `U+00E9` (précomposé) ou
`U+0065 U+0301` (`e` + accent combinant). Affichage identique, octets différents, donc
clés différentes. macOS produit du NFD, Windows et Linux du NFC — d'où un fichier
illisible d'une plateforme à l'autre dès qu'un accent figure dans la phrase de passe.

**Accents, symboles et emoji sont intégralement préservés :** la normalisation ne
supprime rien, elle choisit une écriture unique pour un caractère donné.

**NFC et non NFKC.** La forme de compatibilité `NFKC` fusionne des caractères que
l'utilisateur tient pour distincts (`ﬁ` → `fi`, `²` → `2`, `Ａ` → `A`) et **détruit donc
de l'entropie**. NFC n'unifie que ce qui est canoniquement équivalent.

**Le mot de passe, et lui seul.** Le contenu du document n'est jamais normalisé : il est
stocké octet pour octet, fins de ligne comprises. Le mot de passe n'est jamais restitué,
seulement converti en clé — le normaliser ne perd rien. Le contenu, lui, **est** la donnée
de l'utilisateur. Cas critique : ce coffre stocke des mots de passe ; normaliser un secret
accentué qui y est noté le rendrait invalide sur le service concerné, sans aucun signal.

### 9.5 Argon2id en bref

Un mot de passe porte 30 à 80 bits d'entropie, une clé en exige 256. Le KDF comble cet
écart en rendant chaque tentative **délibérément coûteuse** — le point faible structurel
d'un coffre par mot de passe étant que l'attaquant teste hors ligne, à la vitesse de son
matériel.

Sa spécificité par rapport à PBKDF2 est le **coût mémoire**. Ralentir le calcul ne suffit
pas : un attaquant compense par le parallélisme. Exiger 128 MiB par tentative casse cette
économie — un GPU de 24 Go ne mène qu'une centaine de tentatives simultanées au lieu de
dizaines de milliers, et un ASIC avec RAM embarquée coûte infiniment plus cher.

| Dérivation | Débit attaquant (GPU haut de gamme) |
|---|---|
| `SHA256(mdp)` — ancienne application WPF | ~10⁹ /s |
| Argon2id, 128 MiB, t=4 | ~10² /s |

**L'essentiel du gain de l'axe 4 vient de là**, et non du remplacement d'AES-CBC.

Les trois paramètres : **m** (mémoire, le levier principal), **t** (itérations, du temps
sans plus de RAM), **p** (parallélisme, réduit l'attente sans réduire le coût pour
l'attaquant). Le suffixe **`id`** désigne l'hybride : première moitié en mode `i`
(immunisé aux canaux auxiliaires), seconde en mode `d` (résistance GPU maximale). C'est
la variante recommandée par la RFC 9106 et l'OWASP.

### 9.6 Conséquence pratique : la phrase de passe

C'est là que se joue la sécurité réelle. Face à un attaquant disposant de 10 000 GPU :

| Phrase de passe | Entropie | Temps moyen de cassage |
|---|---|---|
| 3 mots aléatoires | ~39 bits | quelques jours ❌ |
| 4 mots aléatoires | ~52 bits | quelques dizaines d'années |
| 5 mots aléatoires | ~65 bits | au-delà d'un siècle ✅ |
| 6 mots aléatoires | ~78 bits | hors de portée ✅ |

« Aléatoires » au sens diceware : tirés au hasard dans une liste, non choisis par
l'utilisateur. **Cinq mots est l'objectif à recommander dans l'interface.**
