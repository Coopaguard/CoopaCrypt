/**
 * Prévisualisation intégrée à l'éditeur.
 *
 * Une seule vue : le markdown est **rendu sur place**, et la ligne où se trouve
 * le curseur montre son texte source. Écrire reste donc du markdown, sans que
 * la lecture soit encombrée par le balisage.
 *
 * Ce module sépare volontairement deux choses :
 *
 * - [`analyse`], fonction pure qui décide quoi masquer et quoi styler à partir
 *   de l'arbre syntaxique. Elle ne dépend d'aucun DOM et se teste directement.
 * - L'extension CodeMirror, qui se contente de traduire ce résultat en
 *   décorations.
 */

import { markdownParser } from './markdownLang';

export interface Range {
  from: number;
  to: number;
}

export interface StyledRange extends Range {
  cls: string;
}

export interface LineStyle {
  /** Décalage du début de la ligne. */
  pos: number;
  cls: string;
}

export interface Analysis {
  /** Portions de balisage à escamoter. */
  hides: Range[];
  /** Portions de texte à styler. */
  marks: StyledRange[];
  /** Lignes entières à styler. */
  lines: LineStyle[];
  /** Tableaux à remplacer par un rendu, curseur non compris. */
  tables: Range[];
}

/** Marqueurs de balisage escamotés hors de la ligne active. */
const HIDDEN_MARKS = new Set([
  'HeaderMark',
  'EmphasisMark',
  'StrikethroughMark',
  'QuoteMark',
  'LinkMark',
  'URL',
]);

/** Nœuds stylés en tant que fragment de texte. */
const INLINE_STYLES = new Map([
  ['StrongEmphasis', 'cm-lp-strong'],
  ['Emphasis', 'cm-lp-em'],
  ['Strikethrough', 'cm-lp-strike'],
  ['InlineCode', 'cm-lp-code'],
  ['Link', 'cm-lp-link'],
]);

/** Nœuds stylés ligne par ligne. */
const LINE_STYLES = new Map([
  ['ATXHeading1', 'cm-lp-h1'],
  ['ATXHeading2', 'cm-lp-h2'],
  ['ATXHeading3', 'cm-lp-h3'],
  ['ATXHeading4', 'cm-lp-h4'],
  ['ATXHeading5', 'cm-lp-h5'],
  ['ATXHeading6', 'cm-lp-h6'],
  ['Blockquote', 'cm-lp-quote'],
  ['FencedCode', 'cm-lp-fence'],
  ['CodeBlock', 'cm-lp-fence'],
]);

/**
 * Étend une sélection aux lignes entières qu'elle touche.
 *
 * Le source se révèle **par ligne** et non par caractère : masquer le balisage
 * juste devant le curseur ferait sauter le texte à chaque déplacement.
 */
export function activeRange(text: string, selFrom: number, selTo: number): Range {
  const from = text.lastIndexOf('\n', Math.max(0, selFrom - 1)) + 1;
  const nextBreak = text.indexOf('\n', selTo);
  return { from, to: nextBreak === -1 ? text.length : nextBreak };
}

const overlaps = (a: Range, b: Range) => a.from < b.to && b.from < a.to;

/**
 * Décide du rendu de chaque partie du document.
 *
 * `selFrom`/`selTo` délimitent la sélection ; les lignes qu'elle touche gardent
 * leur source visible. Passer `-1, -1` rend tout le document en mode lecture.
 */
export function analyse(text: string, selFrom: number, selTo: number): Analysis {
  const active =
    selFrom < 0 ? { from: -1, to: -1 } : activeRange(text, selFrom, selTo);

  const hides: Range[] = [];
  const marks: StyledRange[] = [];
  const lines: LineStyle[] = [];
  const tables: Range[] = [];

  const tree = markdownParser.parse(text);

  tree.iterate({
    enter: (node) => {
      const range = { from: node.from, to: node.to };
      const revealed = overlaps(range, active);

      const lineStyle = LINE_STYLES.get(node.name);
      if (lineStyle) {
        // Le style de ligne reste appliqué même sur la ligne active : seul le
        // balisage se révèle, la mise en forme ne doit pas clignoter.
        for (const pos of lineStartsWithin(text, range)) {
          lines.push({ pos, cls: lineStyle });
        }
      }

      if (node.name === 'Table') {
        if (!revealed) tables.push(range);
        // Le contenu d'un tableau rendu n'a pas à être décoré en plus.
        if (!revealed) return false;
      }

      const inlineStyle = INLINE_STYLES.get(node.name);
      if (inlineStyle) marks.push({ ...range, cls: inlineStyle });

      if (!revealed && isHidden(node.name, node.node.parent?.name)) {
        hides.push(expandHeaderMark(text, node.name, range));
      }

      return undefined;
    },
  });

  return { hides, marks, lines, tables };
}

function isHidden(name: string, parent: string | undefined): boolean {
  if (name === 'CodeMark') {
    // Les délimiteurs d'un bloc de code clôturé restent visibles : sans eux, on
    // ne verrait plus où le bloc commence ni dans quel langage il est écrit.
    return parent === 'InlineCode';
  }
  if (name === 'URL') {
    // Une adresse n'est escamotée que si elle appartient à un lien : ailleurs,
    // c'est le texte utile.
    return parent === 'Link';
  }
  return HIDDEN_MARKS.has(name);
}

/**
 * Étend l'escamotage d'un `#` aux espaces qui le suivent.
 *
 * Sans cela, le titre rendu commencerait par une indentation parasite.
 */
function expandHeaderMark(text: string, name: string, range: Range): Range {
  if (name !== 'HeaderMark' && name !== 'QuoteMark') return range;
  let to = range.to;
  while (to < text.length && (text[to] === ' ' || text[to] === '\t')) to += 1;
  return { from: range.from, to };
}

function lineStartsWithin(text: string, range: Range): number[] {
  const starts: number[] = [];
  let pos = text.lastIndexOf('\n', Math.max(0, range.from - 1)) + 1;
  while (pos < range.to) {
    starts.push(pos);
    const next = text.indexOf('\n', pos);
    if (next === -1) break;
    pos = next + 1;
  }
  return starts;
}
