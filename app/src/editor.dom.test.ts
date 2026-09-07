// @vitest-environment jsdom

/**
 * Tests montant réellement l'éditeur.
 *
 * Les tests de [`analyse`](./livepreview) vérifient *ce qui devrait* être
 * décoré ; ils ne peuvent rien dire de la façon dont CodeMirror accepte ces
 * décorations. C'est précisément là qu'un bug est passé : un widget de bloc
 * fourni par un plugin de vue est **refusé par CodeMirror**, et l'éditeur
 * plantait dès qu'un document contenait un tableau — sans qu'aucun test unitaire
 * ne s'en aperçoive.
 *
 * Ces tests montent un vrai `EditorView` et se contentent d'exiger qu'il ne
 * lève pas. C'est peu, mais c'est exactement ce qui manquait.
 */

import { EditorSelection } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, describe, expect, it } from 'vitest';

import { createEditor } from './editor';

const TABLE = ['| t1 | T2 |', '| - | - |', '| val1 | val2 |'].join('\n');

const noopCallbacks = {
  onChange: () => {},
  onActivity: () => {},
  onSave: () => {},
  onFind: () => {},
  onFindNext: () => {},
};

let view: EditorView | undefined;

function mount(doc: string, selection?: number): EditorView {
  const parent = document.createElement('div');
  document.body.append(parent);
  const created = createEditor(parent, noopCallbacks);
  created.dispatch({
    changes: { from: 0, to: created.state.doc.length, insert: doc },
    selection: selection === undefined ? undefined : EditorSelection.cursor(selection),
  });
  view = created;
  return created;
}

afterEach(() => {
  view?.destroy();
  view = undefined;
  document.body.replaceChildren();
});

describe('montage de l’éditeur', () => {
  it('accepte un document vide', () => {
    expect(() => mount('')).not.toThrow();
  });

  it('accepte du markdown courant', () => {
    const doc = '# Titre\n\nUn **mot** et `du code`.\n\n> cité\n';
    expect(() => mount(doc)).not.toThrow();
  });

  it('accepte un tableau — la régression qui faisait planter l’éditeur', () => {
    expect(() => mount(TABLE)).not.toThrow();
  });

  it('accepte un tableau suivi de texte', () => {
    expect(() => mount(`${TABLE}\n\nsuite du texte\n`)).not.toThrow();
  });

  it('accepte un tableau sans saut de ligne final', () => {
    // Le nœud syntaxique n'inclut alors pas de fin de ligne : la plage du bloc
    // doit malgré tout couvrir des lignes entières.
    expect(() => mount(TABLE)).not.toThrow();
  });

  it('accepte deux tableaux successifs', () => {
    expect(() => mount(`${TABLE}\n\n${TABLE}\n`)).not.toThrow();
  });

  it('accepte un tableau dans un chapitre', () => {
    expect(() => mount(`# Chapitre\n\n${TABLE}\n`)).not.toThrow();
  });

  it('bascule sans erreur quand le curseur entre dans le tableau', () => {
    const doc = `# Chapitre\n\n${TABLE}\n`;
    const editor = mount(doc);
    const inside = doc.indexOf('val1');
    expect(() => editor.dispatch({ selection: EditorSelection.cursor(inside) })).not.toThrow();
    // Curseur ressorti : le tableau doit se re-rendre sans incident.
    expect(() => editor.dispatch({ selection: EditorSelection.cursor(0) })).not.toThrow();
  });

  it('encaisse une frappe caractère par caractère dans un tableau', () => {
    // Un tableau se construit en tapant : chaque état intermédiaire est
    // syntaxiquement bancal et doit rester inoffensif.
    const editor = mount('');
    expect(() => {
      for (const char of TABLE) {
        editor.dispatch({
          changes: { from: editor.state.doc.length, insert: char },
          selection: EditorSelection.cursor(editor.state.doc.length + 1),
        });
      }
    }).not.toThrow();
    expect(editor.state.doc.toString()).toBe(TABLE);
  });

  it('encaisse la suppression complète d’un document contenant un tableau', () => {
    const editor = mount(`${TABLE}\n`);
    expect(() =>
      editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: '' } }),
    ).not.toThrow();
  });
});
