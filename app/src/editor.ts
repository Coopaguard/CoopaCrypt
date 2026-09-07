/**
 * Éditeur CodeMirror, limité au chapitre courant (`UI.md` §3).
 *
 * L'éditeur ne connaît qu'une **tranche** du document. C'est le modèle
 * (`main.ts`) qui replace cette tranche dans le document complet à chaque
 * modification.
 */

import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorState, type Extension } from '@codemirror/state';
import { EditorView, highlightActiveLine, keymap, lineNumbers } from '@codemirror/view';
import { tags } from '@lezer/highlight';

import { livePreview } from './livepreviewExtension';
import { markdownExtension } from './markdownLang';

/** Coloration sobre : la hiérarchie des titres doit rester lisible d'un coup d'œil. */
const highlight = HighlightStyle.define([
  { tag: tags.heading1, class: 'cm-h1' },
  { tag: tags.heading2, class: 'cm-h2' },
  { tag: tags.heading3, class: 'cm-h3' },
  { tag: tags.strong, fontWeight: 'bold' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.link, class: 'cm-link' },
  { tag: tags.monospace, class: 'cm-code' },
  { tag: tags.list, class: 'cm-list' },
  { tag: tags.quote, class: 'cm-quote' },
]);

export interface EditorCallbacks {
  /** Appelé à chaque modification du texte, avec la tranche complète. */
  onChange: (sliceText: string) => void;
  /** Appelé à toute interaction, pour repousser le verrouillage automatique. */
  onActivity: () => void;
  /** Ctrl+S. */
  onSave: () => void;
  /** Ctrl+F. */
  onFind: () => void;
  /** F3 : occurrence suivante. */
  onFindNext: () => void;
}

/** Raccourci consommant l'événement, pour ne pas le laisser filer au document. */
function keybinding(key: string, action: () => void) {
  return {
    key,
    preventDefault: true,
    run: () => {
      action();
      return true;
    },
  };
}

export function createEditor(parent: HTMLElement, callbacks: EditorCallbacks): EditorView {
  const extensions: Extension[] = [
    lineNumbers(),
    history(),
    highlightActiveLine(),
    markdownExtension,
    syntaxHighlighting(highlight),
    // Rend le markdown sur place ; la ligne du curseur montre son source.
    livePreview(),
    EditorView.lineWrapping,
    keymap.of([
      keybinding('Mod-s', callbacks.onSave),
      keybinding('Mod-f', callbacks.onFind),
      keybinding('F3', callbacks.onFindNext),
      ...historyKeymap,
      ...defaultKeymap,
    ]),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) callbacks.onChange(update.state.doc.toString());
      if (update.docChanged || update.selectionSet) callbacks.onActivity();
    }),
  ];

  return new EditorView({ parent, state: EditorState.create({ extensions }) });
}

/**
 * Remplace le contenu affiché par la tranche d'un autre chapitre.
 *
 * ## Limitation connue
 *
 * Ce remplacement **entre dans l'historique d'annulation** de CodeMirror, alors
 * que changer de chapitre n'est pas une modification du document. Annuler juste
 * après une bascule restaure donc la tranche précédente au lieu de défaire la
 * dernière vraie édition.
 *
 * `UI.md` §3 demande un historique global traversant les chapitres. L'obtenir
 * suppose de tenir la pile d'annulation au niveau du document et non de
 * l'éditeur — un chantier à part entière, pas un réglage. En attendant, cette
 * fonction fait le remplacement le plus simple possible, et le défaut est
 * documenté plutôt que masqué.
 */
export function setSlice(
  view: EditorView,
  text: string,
  selection?: { anchor: number; head: number },
) {
  const anchor = Math.min(selection?.anchor ?? 0, text.length);
  const head = Math.min(selection?.head ?? 0, text.length);
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: text },
    selection: { anchor, head },
    scrollIntoView: true,
  });
}

/** Sélectionne une plage dans la tranche affichée et la fait défiler à l'écran. */
export function revealRange(view: EditorView, from: number, to: number) {
  const max = view.state.doc.length;
  const anchor = Math.min(from, max);
  const head = Math.min(to, max);
  view.dispatch({
    selection: { anchor, head },
    scrollIntoView: true,
  });
  view.focus();
}

/** Position de défilement courante, pour la restaurer en revenant au chapitre. */
export function scrollTop(view: EditorView): number {
  return view.scrollDOM.scrollTop;
}

export function restoreScroll(view: EditorView, top: number) {
  view.scrollDOM.scrollTop = top;
}
