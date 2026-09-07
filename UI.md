# Modèle de navigation et d'édition

> **Statut : proposition, phase 0.** Complète [`FORMAT.md`](FORMAT.md), qui n'est pas
> impacté par ce document. Concerne les axes 1 et 5 de [`evols.md`](evols.md).

## 1. Principe fondateur : le découpage est une vue

Le fichier déchiffré est **un seul document markdown continu**. Les chapitres n'existent
pas dans le stockage : ils sont dérivés à la lecture en analysant les titres.

| | |
|---|---|
| **En mémoire / sur disque** | Un document markdown unique |
| **À l'écran** | Un chapitre à la fois, sélectionné dans le volet |
| **Structure des chapitres** | Déduite des titres, recalculée à chaque frappe |

Ce choix est structurant :

- **Le format de fichier ne change pas.** Aucune structure propriétaire, aucun index à
  maintenir, aucun risque de désynchronisation entre un sommaire stocké et le contenu réel.
- **Le document reste lisible partout.** Déchiffré, c'est du markdown standard, ouvrable
  dans n'importe quel éditeur.
- **La recherche porte sur l'intégralité du document**, pas sur le chapitre affiché. C'est
  ce qui rend possible la navigation vers un résultat situé ailleurs.

## 2. Structure

### 2.1 Ce qui fait un chapitre

Un **titre de niveau 1** (`# Titre`) ouvre un chapitre. Le chapitre s'étend jusqu'au titre
de niveau 1 suivant, ou jusqu'à la fin du document.

Les titres de niveau 2 et au-delà (`##`, `###`) sont des **sous-parties** : elles
apparaissent dans le volet sous leur chapitre, en arborescence, mais ne découpent pas
l'affichage. Cliquer une sous-partie fait défiler à l'intérieur du chapitre courant.

### 2.2 Cas limites

| Situation | Comportement |
|---|---|
| **Aucun titre de niveau 1** | Le document entier forme un chapitre unique, intitulé « Document ». C'est le cas du fichier legacy migré depuis la v1. |
| **Texte avant le premier `#`** | Forme un chapitre implicite en tête, intitulé « Introduction », affiché en premier dans le volet et distingué visuellement (il n'a pas de ligne de titre à éditer). |
| **Titres homonymes** | Autorisés. L'identité d'un chapitre est sa **position ordinale**, jamais son titre (cf. §5). |
| **Titre vide** (`#` seul) | Autorisé. Affiché « (sans titre) » dans le volet. |
| **Document vide** | Un chapitre unique et vide. |
| **`#` dans un bloc de code** | **Ignoré.** L'analyse doit respecter les clôtures ` ``` ` et `~~~`, sinon un script shell commenté découpe le document en morceaux. |

### 2.3 Le titre reste éditable en place

La ligne `# Titre` fait partie du texte éditable du chapitre. Modifier le titre dans
l'éditeur met à jour le volet en direct.

Raison : c'est du markdown, et prétendre le contraire créerait deux chemins de
modification pour la même donnée. Renommer depuis le volet reste possible en confort,
mais l'opération édite simplement cette ligne.

**Conséquence assumée :** supprimer un `#` fusionne deux chapitres, en ajouter un scinde.
C'est le comportement attendu d'un éditeur markdown, et il doit être fluide — pas de
rechargement, pas de perte de position du curseur.

## 3. Édition

L'éditeur n'affiche et ne rend modifiable que la tranche de texte du chapitre courant.

- **La modification s'applique au document complet.** Une édition remplace la tranche
  correspondante dans le document en mémoire ; les décalages des chapitres suivants sont
  recalculés.
- **L'état « modifié » est global**, pas par chapitre : c'est un seul fichier, une seule
  sauvegarde.
- **L'annulation (undo/redo) est globale et traverse les chapitres.** Annuler une action
  faite dans un autre chapitre doit y ramener automatiquement, sinon l'utilisateur voit
  son historique agir sur du contenu invisible.
- **La sauvegarde écrit le document entier**, selon la procédure de `FORMAT.md` §4.1.

## 4. Recherche

C'est le point où le découpage en vues coûte le plus cher en conception, et où il faut
être le plus rigoureux : **des résultats peuvent se trouver dans du contenu non affiché.**

### 4.1 Portée

La recherche porte **toujours sur le document complet**, chapitres masqués inclus. Aucune
option ne restreint la recherche au chapitre courant par défaut ; un filtre « chapitre
courant seulement » peut être proposé, mais désactivé par défaut.

### 4.2 Présentation des résultats

Un simple « suivant » ne suffit pas quand la cible peut être invisible : le saut vers un
autre chapitre serait désorientant sans contexte préalable.

**Le volet affiche donc une liste de résultats groupés par chapitre**, avec le nombre
d'occurrences par chapitre et un extrait de contexte par occurrence. L'utilisateur voit
la répartition avant de naviguer.

`F3` / « suivant » reste disponible et **bascule automatiquement de chapitre** si
nécessaire, en signalant visuellement le changement.

### 4.3 Remplacer

- **Remplacer** agit sur l'occurrence courante, où qu'elle soit.
- **Remplacer tout** agit sur le document complet et doit annoncer le nombre de
  remplacements **et le nombre de chapitres touchés** — modifier silencieusement du
  contenu invisible est inacceptable.
- L'opération doit être annulable en une seule action.

### 4.4 Bugs de la v1 à ne pas reproduire

Repris de [`evols.md`](evols.md) §3.2, à corriger dès l'implémentation :

- **Échapper le texte recherché** s'il n'est pas explicitement en mode expression
  régulière. En v1, chercher `(` lève une exception.
- **Respecter réellement la casse.** En v1, `ReplaceAll` utilise un remplacement de chaîne
  sensible à la casse quelle que soit l'option cochée, et remplace au-delà des
  correspondances trouvées.

## 5. Identité et état de navigation

**L'identité d'un chapitre est sa position ordinale**, jamais son titre : les titres sont
modifiables et peuvent être homonymes. Cette identité est recalculée à chaque analyse et
n'est jamais persistée dans le fichier.

### 5.1 Règle de sécurité

**Aucun élément dérivé du contenu ne sort du fichier chiffré.**

- La configuration de l'application peut mémoriser « dernier chapitre ouvert : index 3 ».
  Elle ne doit **jamais** mémoriser son titre — un titre de chapitre est du contenu de
  coffre.
- Le titre de la fenêtre et l'entrée de barre des tâches ne doivent pas afficher de texte
  dérivé du contenu, pour la même raison.
- Au verrouillage automatique, **le volet se vide au même titre que l'éditeur** : la seule
  arborescence des chapitres est déjà une fuite d'information.

## 6. Objectif de lecture

Le but est le confort de lecture, ce qui impose quelques garde-fous :

- Le volet est **toujours visible** par défaut, et repliable.
- Le chapitre courant est clairement marqué dans le volet.
- Le basculement de chapitre est instantané, sans rechargement ni clignotement.
- La position de défilement de chaque chapitre est mémorisée pendant la session : revenir
  à un chapitre le retrouve là où on l'avait laissé.
- La preview markdown suit le chapitre affiché, elle aussi limitée à celui-ci.

## 7. Points laissés ouverts

- **Réorganisation des chapitres par glisser-déposer** dans le volet. Séduisant, mais
  cela revient à déplacer des blocs de texte dans le document et demande une gestion
  d'annulation soignée. À traiter après la phase 3, pas pendant.
- **Repli des sous-parties** dans le volet : utile au-delà d'une vingtaine de chapitres,
  probablement superflu en deçà.
