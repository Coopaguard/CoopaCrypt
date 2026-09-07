/**
 * Traduction de l'analyse markdown en décorations CodeMirror.
 *
 * Toute la logique de décision vit dans [`analyse`](./livepreview) ; ce module
 * ne fait que la rendre.
 *
 * ## Pourquoi un `StateField` et non un `ViewPlugin`
 *
 * CodeMirror **refuse les décorations de bloc issues d'un plugin de vue** et
 * lève une exception à la première rencontre. Or un tableau est remplacé par un
 * widget de bloc : l'éditeur plantait donc dès qu'un document contenait un
 * tableau. Les décorations sont produites ici par un champ d'état, seul endroit
 * où le remplacement de bloc est autorisé.
 */

import { RangeSet, StateField, type EditorState, type Range as CmRange } from '@codemirror/state';
import { Decoration, type DecorationSet, EditorView, WidgetType } from '@codemirror/view';

import { analyse, type Range } from './livepreview';
import { renderMarkdown } from './preview';

/** Escamotage : la portion reste dans le document, elle n'est plus affichée. */
const hideMark = Decoration.replace({});

/**
 * Tableau rendu à la place de son source.
 *
 * Le HTML passe par le même assainissement que le reste : le contenu du coffre
 * est rendu dans une vue qui a accès au pont IPC, et rien ne garantit qu'un
 * coffre soit resté sous le seul contrôle de son auteur.
 */
class TableWidget extends WidgetType {
  constructor(
    private readonly source: string,
    private readonly from: number,
  ) {
    super();
  }

  eq(other: TableWidget): boolean {
    return other.source === this.source && other.from === this.from;
  }

  toDOM(view: EditorView): HTMLElement {
    const wrapper = document.createElement('div');
    wrapper.className = 'cm-lp-table';
    wrapper.innerHTML = renderMarkdown(this.source);

    // Un clic ramène le curseur dans le tableau, ce qui en révèle le source :
    // sans cela, un tableau rendu ne serait plus modifiable.
    wrapper.addEventListener('mousedown', (event) => {
      event.preventDefault();
      view.dispatch({ selection: { anchor: this.from }, scrollIntoView: true });
      view.focus();
    });

    return wrapper;
  }

  /** Le widget gère lui-même ses événements. */
  ignoreEvent(): boolean {
    return true;
  }
}

/**
 * Aligne une plage sur des lignes entières.
 *
 * Une décoration de bloc doit couvrir des lignes complètes ; une plage qui
 * s'arrête au milieu d'une ligne fait échouer la construction. Selon la forme
 * du tableau, le nœud syntaxique inclut ou non le saut de ligne final, d'où
 * cette normalisation.
 */
function wholeLines(state: EditorState, range: Range): Range | null {
  const max = state.doc.length;
  const from = Math.max(0, Math.min(range.from, max));
  const to = Math.max(from, Math.min(range.to, max));
  if (to <= from) return null;

  const startLine = state.doc.lineAt(from);
  // `to - 1` garde la recherche à l'intérieur du tableau : si la plage se
  // termine sur un saut de ligne, `lineAt(to)` désignerait la ligne suivante.
  const endLine = state.doc.lineAt(Math.max(from, to - 1));

  return { from: startLine.from, to: endLine.to };
}

function build(state: EditorState): DecorationSet {
  const text = state.doc.toString();
  const { main } = state.selection;
  const { hides, marks, lines, tables } = analyse(text, main.from, main.to);

  const ranges: CmRange<Decoration>[] = [];
  const rendered: Range[] = [];

  for (const table of tables) {
    const span = wholeLines(state, table);
    if (!span) continue;
    const widget = new TableWidget(text.slice(span.from, span.to), span.from);
    ranges.push(Decoration.replace({ widget, block: true }).range(span.from, span.to));
    rendered.push(span);
  }

  // Rien ne doit décorer l'intérieur d'un bloc remplacé : les décorations s'y
  // chevaucheraient sans jamais être affichées.
  const insideTable = (from: number, to: number) =>
    rendered.some((span) => from < span.to && span.from < to);

  for (const line of lines) {
    if (insideTable(line.pos, line.pos + 1)) continue;
    ranges.push(Decoration.line({ class: line.cls }).range(line.pos));
  }
  for (const mark of marks) {
    if (mark.to <= mark.from || insideTable(mark.from, mark.to)) continue;
    ranges.push(Decoration.mark({ class: mark.cls }).range(mark.from, mark.to));
  }
  for (const hide of hides) {
    if (hide.to <= hide.from || insideTable(hide.from, hide.to)) continue;
    ranges.push(hideMark.range(hide.from, hide.to));
  }

  // Tri demandé : les décorations viennent de plusieurs passes et ne sont pas
  // naturellement ordonnées.
  return RangeSet.of(ranges, true);
}

/**
 * Extension de prévisualisation intégrée.
 *
 * Les décorations sont recalculées à chaque modification **et à chaque
 * déplacement du curseur**, puisque c'est la position du curseur qui décide
 * quelles lignes montrent leur source.
 */
export const livePreviewField = StateField.define<DecorationSet>({
  create: (state) => build(state),
  update: (value, tr) => (tr.docChanged || tr.selection ? build(tr.state) : value),
  provide: (field) => EditorView.decorations.from(field),
});

export function livePreview() {
  return livePreviewField;
}
