/**
 * Recherche et remplacement (`UI.md` §4).
 *
 * La recherche porte **toujours sur le document entier**, chapitres masqués
 * inclus. C'est la contrepartie obligatoire du fait de n'afficher qu'un
 * chapitre à la fois : sans cela, un résultat situé ailleurs serait
 * introuvable.
 */

import type { Chapter } from './structure';
import { chapterAt } from './structure';

export interface Match {
  /** Décalage absolu du début de l'occurrence dans le document complet. */
  from: number;
  /** Décalage absolu de fin (exclu). */
  to: number;
  /** Index du chapitre qui contient l'occurrence. */
  chapterIndex: number;
  /** Extrait de contexte, pour afficher le résultat sans ouvrir le chapitre. */
  excerpt: string;
}

export interface ChapterMatches {
  chapterIndex: number;
  chapterTitle: string;
  matches: Match[];
}

const EXCERPT_RADIUS = 32;

/**
 * Cherche toutes les occurrences d'un terme dans le document.
 *
 * Le terme est traité comme du **texte littéral**, jamais comme une expression
 * régulière : c'est le bug de la v1, où chercher `(` levait une exception
 * (`UI.md` §4.4).
 */
export function findAll(
  text: string,
  needle: string,
  chapters: Chapter[],
  caseSensitive: boolean,
): Match[] {
  if (needle.length === 0) return [];

  const haystack = caseSensitive ? text : text.toLowerCase();
  const target = caseSensitive ? needle : needle.toLowerCase();

  const matches: Match[] = [];
  let cursor = 0;

  for (;;) {
    const at = haystack.indexOf(target, cursor);
    if (at === -1) break;

    matches.push({
      from: at,
      to: at + needle.length,
      chapterIndex: chapterAt(chapters, at).index,
      excerpt: excerptAround(text, at, at + needle.length),
    });

    // Avancer d'au moins un caractère : un terme peut se chevaucher lui-même
    // (« aa » dans « aaa »), et repartir de `at` boucherait indéfiniment.
    cursor = at + Math.max(needle.length, 1);
  }

  return matches;
}

/** Regroupe les occurrences par chapitre, pour l'affichage du volet. */
export function groupByChapter(matches: Match[], chapters: Chapter[]): ChapterMatches[] {
  const groups = new Map<number, Match[]>();
  for (const match of matches) {
    const list = groups.get(match.chapterIndex);
    if (list) list.push(match);
    else groups.set(match.chapterIndex, [match]);
  }

  return [...groups.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([chapterIndex, list]) => ({
      chapterIndex,
      chapterTitle: chapters[chapterIndex]?.title ?? '(chapitre inconnu)',
      matches: list,
    }));
}

/**
 * Occurrence suivante à partir d'une position, en repartant au début une fois
 * la fin atteinte.
 */
export function nextMatch(matches: Match[], after: number): Match | null {
  if (matches.length === 0) return null;
  return matches.find((m) => m.from > after) ?? matches[0];
}

/**
 * Remplace toutes les occurrences dans le document complet.
 *
 * Rend aussi le nombre de chapitres touchés : modifier silencieusement du
 * contenu invisible est inacceptable, l'interface doit pouvoir l'annoncer
 * (`UI.md` §4.3).
 */
export function replaceAll(
  text: string,
  matches: Match[],
  replacement: string,
): { text: string; count: number; chaptersTouched: number } {
  if (matches.length === 0) {
    return { text, count: 0, chaptersTouched: 0 };
  }

  // Reconstruction par tranches plutôt que `String.replace` : c'est ce qui
  // garantit que seules les occurrences trouvées sont remplacées, en respectant
  // réellement l'option de casse — l'autre bug de la v1 (`UI.md` §4.4).
  const pieces: string[] = [];
  let cursor = 0;
  for (const match of matches) {
    pieces.push(text.slice(cursor, match.from), replacement);
    cursor = match.to;
  }
  pieces.push(text.slice(cursor));

  return {
    text: pieces.join(''),
    count: matches.length,
    chaptersTouched: new Set(matches.map((m) => m.chapterIndex)).size,
  };
}

function excerptAround(text: string, from: number, to: number): string {
  const start = Math.max(0, from - EXCERPT_RADIUS);
  const end = Math.min(text.length, to + EXCERPT_RADIUS);
  const prefix = start > 0 ? '…' : '';
  const suffix = end < text.length ? '…' : '';
  // Les retours à la ligne casseraient l'affichage sur une seule ligne.
  return prefix + text.slice(start, end).replace(/\s+/g, ' ') + suffix;
}
