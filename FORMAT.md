# Format de fichier `.coocrypt` — spécification

> **Statut : gelé.** Implémentation de référence :
> [`crates/coopacrypt-core`](crates/coopacrypt-core) — vecteurs de test produits (§5).
> Le format ne peut plus changer sans incrémenter le numéro de version de l'en-tête.

## 1. Principes directeurs

| Principe | Conséquence sur le format |
|---|---|
| **Le fichier est autosuffisant** | Tous les paramètres de dérivation sont stockés en clair dans l'en-tête. Aucune information externe n'est nécessaire pour ouvrir le fichier. |
| **Portable sans infrastructure** | Aucune dépendance à un trousseau système, un serveur ou une base locale. Une copie sur clé USB ou via un cloud reste exploitable. |
| **Chiffrement authentifié** | Toute altération du fichier, en-tête compris, est détectée au déchiffrement. |
| **Agilité cryptographique** | Les identifiants de KDF et d'AEAD sont des champs versionnés : un futur algorithme s'ajoute sans casser l'existant. |
| **Fuite de métadonnées minimale** | Le clair est complété par du remplissage pour masquer sa longueur exacte. |

## 2. Choix cryptographiques

### 2.1 Dérivation de clé — Argon2id

Retenu comme lauréat de la Password Hashing Competition et recommandé par l'OWASP.
La variante `id` combine la résistance aux attaques par canal auxiliaire de `i` et la
résistance GPU/ASIC de `d`.

**Paramètres par défaut à la création :**

| Paramètre | Valeur | Justification |
|---|---|---|
| **Version d'Argon2** | **0x13 (v1.3)** | **Obligatoire.** Argon2 existe en version `0x10` et `0x13`, qui produisent des sorties différentes. Les bibliothèques ne partagent pas toutes le même défaut : ne jamais s'en remettre au défaut, toujours forcer `0x13`. |
| Coût mémoire | 131 072 KiB (128 MiB) | Compromis entre coût pour l'attaquant et compatibilité mobile. Une allocation supérieure risque une éviction par le système sur iOS. |
| Itérations (coût temps) | 4 | Mesuré à ~0,25 s par dérivation en build `release` sur un poste de bureau récent. |
| Parallélisme (lanes) | 4 | |
| Longueur de sortie | 32 octets | Clé de 256 bits. |
| Longueur du sel | 16 octets | Aléatoire cryptographique, unique par fichier. |
| Clé secrète (paramètre `K`) | **vide** | Argon2 admet une clé secrète optionnelle. Elle n'est pas utilisée : elle romprait l'autosuffisance du fichier. |
| Données associées (paramètre `X`) | **vides** | Idem, non utilisées. |

Les deux derniers paramètres sont explicités parce qu'ils sont optionnels dans Argon2 :
une implémentation qui y injecterait une valeur non vide produirait des fichiers
incompatibles, sans qu'aucun champ de l'en-tête ne le signale.

**Mot de passe vide.** Interdit. L'implémentation **doit** refuser un mot de passe vide à
l'écriture comme à la lecture, avant toute dérivation.

Ces paramètres sont **stockés dans l'en-tête**. Un fichier créé avec des paramètres plus
faibles ou plus forts reste lisible : l'implémentation lit les valeurs du fichier.

**Durcissement automatique.** À l'écriture, l'implémentation utilise toujours les
paramètres par défaut de sa version courante, jamais ceux lus dans le fichier. Comme
chaque enregistrement tire un nouveau sel et impose donc une re-dérivation, un fichier
ancien est silencieusement durci à la première sauvegarde. Aucune interaction utilisateur
n'est nécessaire.

### 2.1.1 Pourquoi la variante `id`

| Variante | Accès mémoire | Propriété |
|---|---|---|
| Argon2d | dépendants des données | Résistance GPU maximale, mais les motifs d'accès dépendent du secret : vulnérable aux canaux auxiliaires. |
| Argon2i | indépendants des données | Immunisé aux canaux auxiliaires, résistance GPU plus faible. |
| **Argon2id** | hybride | Première moitié en mode `i`, seconde en mode `d`. Retenu par la RFC 9106 et l'OWASP. |

### 2.1.2 Rôle du coût mémoire

Un KDF purement calculatoire (PBKDF2) se contourne par le parallélisme : un GPU aligne
des milliers de cœurs, un ASIC des millions. Le coût mémoire casse cette économie — un
GPU de 24 Go ne peut mener que quelques centaines de tentatives simultanées à 128 MiB chacune,
au lieu de dizaines de milliers.

Ordre de grandeur du débit d'un attaquant disposant d'un GPU haut de gamme :

| Dérivation | Candidats testés par seconde |
|---|---|
| `SHA256(mot_de_passe)` — ancienne application WPF | ~10⁹ |
| Argon2id, 128 MiB, t=4 | ~10² |

L'essentiel du gain de sécurité provient de ce facteur, et non du remplacement du
chiffre lui-même.

> **Note de sécurité.** Le niveau de protection réel est plafonné par l'entropie du mot
> de passe, pas par le choix de l'AEAD. Face à un attaquant disposant de 10 000 GPU, une
> phrase de passe de trois mots aléatoires tombe en quelques jours ; cinq mots la placent
> hors de portée. L'application doit activement encourager les phrases de passe longues.

### 2.1.3 Normalisation du mot de passe — NFC

Le mot de passe est **normalisé en NFC (RFC 8265, profil PRECIS `OpaqueString`)** puis
encodé en UTF-8 avant d'être passé à Argon2id.

**Motif.** Unicode admet plusieurs représentations d'un même caractère : « é » s'écrit
soit `U+00E9` (précomposé, `C3 A9`), soit `U+0065 U+0301` (`e` + accent combinant,
`65 CC 81`). Les deux sont de l'UTF-8 valide et s'affichent identiquement, mais
produisent des octets — donc des clés — différents. macOS produit historiquement du NFD,
Windows et Linux du NFC : sans normalisation, un fichier créé sur une plateforme est
illisible sur l'autre dès que la phrase de passe contient un accent.

**Pourquoi NFC et non NFKC.** La forme de compatibilité `NFKC` fusionne des caractères
que l'utilisateur considère comme distincts — la ligature `ﬁ` devient `fi`, l'exposant
`²` devient `2`, la pleine chasse `Ａ` devient `A`. Elle **détruit de l'entropie** en
rendant identiques des mots de passe différents. NFC n'unifie que les représentations
canoniquement équivalentes, c'est-à-dire strictement le même caractère écrit autrement.

Accents, symboles, emoji et casse sont donc intégralement préservés.

**Portée : le mot de passe, et lui seul.** Le contenu du document n'est **jamais**
normalisé — il est stocké et restitué octet pour octet.

Cette asymétrie est délibérée et importante :

- Le mot de passe n'est jamais restitué à l'utilisateur, seulement comparé implicitement
  via la clé qu'il produit. Le normaliser corrige une divergence de saisie entre systèmes
  sans rien perdre.
- Le contenu, lui, **est** la donnée de l'utilisateur. Le normaliser reviendrait à la
  modifier silencieusement. Le cas critique est direct : ce coffre sert à stocker des
  mots de passe. Normaliser un secret accentué noté dans le document le rendrait invalide
  sur le service auquel il donne accès, sans aucun signal pour l'utilisateur.

**Fins de ligne.** Non normalisées non plus : elles font partie du contenu et sont
conservées telles quelles. L'application de référence écrit du `LF` pour les documents
qu'elle crée, mais ne réécrit jamais les fins de ligne d'un document existant.

### 2.2 Chiffrement authentifié — XChaCha20-Poly1305

Retenu plutôt qu'AES-256-GCM pour deux raisons :

1. **Nonce de 192 bits.** Il peut être tiré aléatoirement sans risque pratique de
   collision. AES-GCM, avec ses 96 bits, impose un compteur rigoureux ; sa réutilisation
   de nonce est catastrophique. Sur un format où chaque enregistrement retire un nonce,
   cette marge élimine une classe entière d'erreurs.
2. **Pas de dépendance à l'accélération matérielle.** ChaCha20 est rapide et à temps
   constant en logiciel pur, y compris sur les plateformes sans AES-NI.

Les deux algorithmes offrent un niveau de sécurité équivalent (~128 bits) contre toutes
les attaques connues.

**Chiffrement en cascade : écarté.** Superposer deux AEAD couvre un risque théorique
quasi nul (la rupture d'une primitive moderne) au prix d'un doublement de la surface de
code cryptographique, donc du risque réel — l'erreur d'implémentation.

### 2.3 Remplissage (padding)

Le clair est complété jusqu'au multiple de **4096 octets** supérieur. Un fichier
synchronisé expose sa taille à l'hébergeur ; sans remplissage, cette taille révèle la
longueur exacte du contenu et son évolution dans le temps.

## 3. Structure du fichier — version 2

### 3.1 Vue d'ensemble

```
+---------------------------+
|  En-tête (64 octets)      |  en clair, authentifié comme AAD
+---------------------------+
|  Chiffré (N × 4096)       |
+---------------------------+
|  Tag Poly1305 (16 octets) |
+---------------------------+
```

Taille totale = 64 + (N × 4096) + 16 octets, avec N ≥ 1.
Taille minimale : 4176 octets.

### 3.2 En-tête

Tous les entiers sont en **big-endian**.

| Offset | Taille | Champ | Valeur |
|---:|---:|---|---|
| 0 | 8 | Magic | `COOCRYPT` (ASCII, `43 4F 4F 43 52 59 50 54`) |
| 8 | 1 | Version du format | `0x02` |
| 9 | 1 | Identifiant KDF | `0x01` = Argon2id |
| 10 | 1 | Identifiant AEAD | `0x01` = XChaCha20-Poly1305 |
| 11 | 1 | Réservé | `0x00` |
| 12 | 4 | Argon2id — coût mémoire (KiB) | u32 |
| 16 | 4 | Argon2id — itérations | u32 |
| 20 | 1 | Argon2id — parallélisme | u8 |
| 21 | 3 | Réservé | `00 00 00` |
| 24 | 16 | Sel | aléatoire |
| 40 | 24 | Nonce | aléatoire |
| **64** | | *fin de l'en-tête* | |

Les octets réservés **doivent** valoir zéro à l'écriture et **doivent** être rejetés
s'ils sont non nuls à la lecture : cela réserve leur usage pour de futures extensions.

### 3.3 Données authentifiées additionnelles (AAD)

Les **64 octets de l'en-tête** sont passés intégralement comme AAD à l'AEAD. Toute
modification de l'en-tête — abaissement des paramètres Argon2id, substitution du sel,
changement d'identifiant d'algorithme — invalide le tag et fait échouer le déchiffrement.

### 3.4 Structure du clair avant chiffrement

```
+--------+------------------------+------------------------+
| u32 BE |      contenu UTF-8     |    remplissage à zéro  |
| long.  |                        |                        |
+--------+------------------------+------------------------+
|<------------ multiple de 4096 octets ------------------->|
```

| Champ | Taille | Description |
|---|---|---|
| Longueur | 4 octets | Taille en octets du contenu, u32 big-endian |
| Contenu | variable | Document markdown, UTF-8, sans BOM, **conservé octet pour octet** (ni normalisation Unicode, ni conversion de fins de ligne — cf. §2.1.3) |
| Remplissage | variable | Octets nuls jusqu'au multiple de 4096 supérieur |

À la lecture, le remplissage est ignoré : seuls les `longueur` premiers octets qui suivent
le champ de longueur constituent le document. Les octets de remplissage ne sont pas
vérifiés — le tag couvre déjà leur intégrité, et les laisser libres préserve une marge
pour de futures extensions.

**Validation obligatoire à la lecture :** `longueur ≤ taille_du_clair − 4`. Un champ de
longueur incohérent doit provoquer un rejet, jamais une lecture au-delà du tampon.

Le contenu maximal est de 2³² − 1 octets, très au-delà de tout usage réel.

## 4. Procédures

### 4.1 Écriture

1. Tirer un sel de 16 octets et un nonce de 24 octets avec un générateur cryptographique.
2. Normaliser le mot de passe en NFC, puis l'encoder en UTF-8.
3. Dériver la clé : `clé = Argon2id(mot_de_passe, sel, m, t, p, 32 octets)`, en utilisant
   **les paramètres par défaut de la version courante**, non ceux du fichier d'origine.
4. Construire l'en-tête de 64 octets avec les paramètres effectivement utilisés.
5. Construire le clair : `u32(len) || contenu || remplissage`.
6. Chiffrer : `(chiffré, tag) = XChaCha20-Poly1305(clé, nonce, clair, aad = en-tête)`.
7. Écrire `en-tête || chiffré || tag` **de façon atomique** : écriture dans un fichier
   temporaire du même répertoire, `fsync`, puis remplacement. Indispensable dès lors que
   le fichier réside dans un dossier synchronisé.
8. Effacer la clé et le mot de passe de la mémoire (`zeroize`).

### 4.2 Lecture

1. Vérifier la taille minimale (4176 octets) et la congruence : `(taille − 80) mod 4096 == 0`.
2. Vérifier le magic. S'il est absent, rejeter immédiatement : le fichier n'est pas un
   coffre, et il serait absurde de faire payer une dérivation Argon2id à l'utilisateur.
3. Vérifier que la version, les identifiants KDF et AEAD sont supportés, et que les
   octets réservés sont nuls.
4. **Valider les paramètres Argon2id contre des bornes de sûreté** avant de dériver :
   un en-tête hostile annonçant 64 GiB de mémoire provoquerait un déni de service.
   Bornes proposées : mémoire ≤ 4 GiB, itérations ≤ 32, parallélisme ≤ 16.
   Une implémentation **peut** appliquer des bornes plus basses selon sa plateforme —
   une compilation 32 bits ne peut de toute façon pas allouer 4 GiB — mais elle doit
   alors rapporter une erreur explicite de type « paramètres non supportés sur cette
   plateforme », et surtout pas « mot de passe incorrect ».
5. Rejeter un mot de passe vide, puis le normaliser en NFC et l'encoder en UTF-8.
6. Dériver la clé avec **les paramètres lus dans l'en-tête**, puis déchiffrer et
   **vérifier le tag**.
7. En cas d'échec du tag : signaler « mot de passe incorrect ou fichier altéré ».
   Ne jamais exploiter le clair d'un tag invalide.
8. Valider le champ de longueur (`longueur ≤ taille_du_clair − 4`) **avant** de découper,
   puis extraire le contenu.

### 4.3 Distinction des erreurs

Le tag ne permet pas de séparer « mauvais mot de passe » de « fichier corrompu » — c'est
une propriété voulue, et le message utilisateur doit couvrir les deux cas. En revanche,
un magic ou une taille invalides sont détectés avant toute dérivation et peuvent donner
un message distinct et immédiat.

## 5. Vecteurs de test

> **Produits et gelés.** Fichier :
> [`crates/coopacrypt-core/tests/vectors.json`](crates/coopacrypt-core/tests/vectors.json)
> — 13 vecteurs. Toute implémentation, dans n'importe quel langage, doit les reproduire
> à l'octet près.

Chaque vecteur fixe : mot de passe (brut **et** sous sa forme préparée en hexadécimal),
sel, nonce, paramètres Argon2id, contenu en clair (en hexadécimal, pour lever toute
ambiguïté d'échappement) et le fichier complet attendu en hexadécimal.

Génération : `cargo run --release --example gen_vectors`.
Vérification : `cargo test`.

> **Régénérer ces vecteurs revient à modifier le format.** Si un test de conformité
> échoue, le réflexe correct est de chercher ce qui a bougé dans le code — jamais de
> relancer le générateur pour faire disparaître l'échec.

Cas couverts :

| Vecteur | Ce qu'il verrouille |
|---|---|
| `contenu-vide` | Fichier minimal : 4176 octets |
| `ascii-court` | Cas nominal |
| `utf8-multioctets` | Accents, emoji, idéogrammes |
| `contenu-nfd-preserve` | Le contenu n'est **pas** normalisé |
| `contenu-crlf-preserve` | Les fins de ligne traversent intactes |
| `frontiere-4092` | Dernier contenu tenant dans un bloc |
| `frontiere-4093` | Bascule sur deux blocs |
| `mdp-nfc` / `mdp-nfd` | NFC et NFD du mot de passe convergent |
| `mdp-espace-insecable` | `U+00A0` ramené à l'espace ASCII |
| `mdp-ligature` / `mdp-ligature-temoin` | NFKC **n'est pas** appliqué : `ﬁn` ≠ `fin` |
| `parametres-par-defaut` | Fige les paramètres réels : 128 MiB, t=4, p=4 |

Cas d'échec vérifiés par la suite de tests du crate (hors `vectors.json`) : tag altéré,
en-tête altéré, octet réservé non nul, paramètres Argon2id hors bornes, taille de fichier
non congruente, champ de longueur supérieur à la taille du clair, mot de passe vide,
abaissement des paramètres dans l'en-tête.

## 6. Changement de mot de passe

**Re-chiffrement complet.** Déchiffrement en mémoire, nouveau sel, nouveau nonce,
nouvelle dérivation, réécriture atomique du fichier.

L'alternative — une clé de fichier aléatoire enveloppée par la clé dérivée du mot de
passe, permettant de ne réécrire que l'enveloppe — a été écartée. Sur un fichier de cette
taille, le gain de performance est nul, et l'indirection ajoute un chemin de
déchiffrement supplémentaire à sécuriser. Moins de code sur le chemin critique.

## 7. Points laissés ouverts

*Aucun à ce stade. Les arbitrages de phase 0 sont clos ; la spécification est prête à
être implémentée.*
