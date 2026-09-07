/**
 * Instance unique du langage markdown, partagée par l'éditeur, le découpage en
 * chapitres et la prévisualisation intégrée.
 *
 * `markdownLanguage` est la variante **GFM** : elle ajoute les tableaux, les
 * listes de tâches et le texte barré à CommonMark. Le choix compte au-delà du
 * confort — sans elle, un tableau n'existe pas dans l'arbre syntaxique et ne
 * peut donc pas être rendu.
 *
 * Un seul point de définition évite que l'analyse des chapitres et celle de
 * l'affichage divergent sur ce que le document contient.
 */

import { markdown, markdownLanguage } from '@codemirror/lang-markdown';

/** Extension à installer dans l'éditeur. */
export const markdownExtension = markdown({ base: markdownLanguage });

/** Analyseur utilisable hors éditeur, notamment dans les tests. */
export const markdownParser = markdownLanguage.parser;
