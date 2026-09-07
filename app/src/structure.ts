/**
 * Découpage du document en chapitres et sous-parties (`UI.md` §2).
 *
 * Le document reste **un seul markdown continu** ; ce module en dérive une vue.
 * Rien n'est stocké : la structure est recalculée à partir du texte.
 *
 * L'analyse s'appuie sur l'arbre syntaxique de `@codemirror/lang-markdown`
 * (Lezer) et non sur une expression régulière. C'est ce qui fait qu'un `#` au
 * sein d'un bloc de code clôturé ne découpe pas le document — le cas que
 * `UI.md` §2.2 signale explicitement, et qu'un `^#` naïf manquerait.
 */

import type { SyntaxNode } from '@lezer/common';

import { markdownParser } from './markdownLang';

/** Un titre repéré dans le document. */
export interface Heading {
  /** Niveau du titre : 1 pour `#`, 2 pour `##`, etc. */
  level: number;
  /** Texte du titre, sans les dièses ni les espaces qui les suivent. */
  title: string;
  /** Décalage, en caractères, du début de la ligne de titre. */
  from: number;
}

/** Une sous-partie, c'est-à-dire un titre de niveau 2 ou plus. */
export interface Subsection {
  level: number;
  title: string;
  /** Décalage absolu dans le document complet. */
  from: number;
}

/** Un chapitre : la tranche de document ouverte par un titre de niveau 1. */
export interface Chapter {
  /**
   * Identité du chapitre : sa **position ordinale** (`UI.md` §5).
   *
   * Jamais son titre — les titres sont modifiables et peuvent être homonymes.
   */
  index: number;
  /** Titre affiché dans le volet. */
  title: string;
  /** Décalage de début de la tranche dans le document complet. */
  from: number;
  /** Décalage de fin (exclu) de la tranche dans le document complet. */
  to: number;
  /**
   * Vrai pour le chapitre implicite formé par le texte précédant le premier
   * titre de niveau 1. Il n'a pas de ligne de titre à éditer (`UI.md` §2.2).
   */
  implicit: boolean;
  /** Sous-parties du chapitre, décalages absolus. */
  subsections: Subsection[];
}

const PREAMBLE_TITLE = 'Introduction';
const SINGLE_TITLE = 'Document';
const UNTITLED = '(sans titre)';

/** Noms de nœuds Lezer des titres ATX, par niveau. */
const ATX_HEADINGS = new Map<string, number>([
  ['ATXHeading1', 1],
  ['ATXHeading2', 2],
  ['ATXHeading3', 3],
  ['ATXHeading4', 4],
  ['ATXHeading5', 5],
  ['ATXHeading6', 6],
]);

/**
 * Relève tous les titres ATX du document, blocs de code exclus.
 *
 * Les titres « setext » (soulignés par `===` ou `---`) ne sont volontairement
 * pas pris en compte : l'application n'en produit pas, et les traiter
 * imposerait de gérer leur transformation à l'édition ligne par ligne.
 */
export function findHeadings(text: string): Heading[] {
  const tree = markdownParser.parse(text);
  const headings: Heading[] = [];

  tree.iterate({
    enter: (node) => {
      const level = ATX_HEADINGS.get(node.name);
      if (level === undefined) return;
      headings.push({
        level,
        title: headingText(text, node.node),
        from: node.from,
      });
    },
  });

  headings.sort((a, b) => a.from - b.from);
  return headings;
}

/** Extrait le texte d'un titre en retirant les dièses de balisage. */
function headingText(text: string, node: SyntaxNode): string {
  const raw = text.slice(node.from, node.to);
  // `HeaderMark` porte les dièses ; les retirer par le texte est plus simple et
  // donne le même résultat, y compris pour la forme fermée `## Titre ##`.
  return raw.replace(/^#{1,6}\s*/, '').replace(/\s+#+\s*$/, '').trim();
}

/**
 * Découpe le document en chapitres.
 *
 * Garantit toujours **au moins un chapitre**, y compris pour un document vide :
 * l'interface n'a alors aucun cas particulier à traiter.
 */
export function splitChapters(text: string): Chapter[] {
  const headings = findHeadings(text);
  const tops = headings.filter((h) => h.level === 1);

  // Aucun titre de niveau 1 : le document entier forme un chapitre unique
  // (`UI.md` §2.2). C'est le cas d'un coffre neuf ou d'un document non structuré.
  if (tops.length === 0) {
    return [
      {
        index: 0,
        title: SINGLE_TITLE,
        from: 0,
        to: text.length,
        implicit: true,
        subsections: subsectionsWithin(headings, 0, text.length),
      },
    ];
  }

  const chapters: Chapter[] = [];

  // Texte précédant le premier `#` : chapitre implicite en tête.
  if (tops[0].from > 0) {
    const to = tops[0].from;
    chapters.push({
      index: 0,
      title: PREAMBLE_TITLE,
      from: 0,
      to,
      implicit: true,
      subsections: subsectionsWithin(headings, 0, to),
    });
  }

  tops.forEach((heading, i) => {
    const from = heading.from;
    const to = i + 1 < tops.length ? tops[i + 1].from : text.length;
    chapters.push({
      index: chapters.length,
      title: heading.title || UNTITLED,
      from,
      to,
      implicit: false,
      subsections: subsectionsWithin(headings, from, to),
    });
  });

  return chapters;
}

function subsectionsWithin(headings: Heading[], from: number, to: number): Subsection[] {
  return headings
    .filter((h) => h.level >= 2 && h.from >= from && h.from < to)
    .map((h) => ({
      level: h.level,
      title: h.title || UNTITLED,
      from: h.from,
    }));
}

/**
 * Localise le chapitre contenant un décalage absolu.
 *
 * Sert à faire basculer la vue vers un résultat de recherche situé dans un
 * chapitre non affiché (`UI.md` §4.2).
 */
export function chapterAt(chapters: Chapter[], offset: number): Chapter {
  for (const chapter of chapters) {
    if (offset >= chapter.from && offset < chapter.to) return chapter;
  }
  // Un décalage en fin de document tombe hors de toute tranche : rattacher au
  // dernier chapitre plutôt que d'échouer.
  return chapters[chapters.length - 1];
}

/**
 * Remplace la tranche d'un chapitre dans le document complet (`UI.md` §3).
 *
 * L'édition porte toujours sur le document entier ; l'affichage d'une seule
 * tranche est une vue.
 */
export function replaceChapter(text: string, chapter: Chapter, replacement: string): string {
  return text.slice(0, chapter.from) + replacement + text.slice(chapter.to);
}
