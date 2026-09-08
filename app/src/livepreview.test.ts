import { describe, expect, it } from 'vitest';
import { activeRange, analyse } from './livepreview';

/** Texte effectivement escamoté, pour lire les attentes en clair. */
const hidden = (text: string, selFrom = -1, selTo = -1) =>
  analyse(text, selFrom, selTo).hides.map((h) => text.slice(h.from, h.to));

const lineClasses = (text: string, selFrom = -1, selTo = -1) =>
  analyse(text, selFrom, selTo).lines.map((l) => l.cls);

const markClasses = (text: string, selFrom = -1, selTo = -1) =>
  analyse(text, selFrom, selTo).marks.map((m) => m.cls);

describe('ligne active', () => {
  it('s’étend à la ligne entière touchée par le curseur', () => {
    const text = 'alpha\nbeta\ngamma';
    expect(activeRange(text, 7, 7)).toEqual({ from: 6, to: 10 });
  });

  it('couvre toutes les lignes d’une sélection multiligne', () => {
    const text = 'alpha\nbeta\ngamma';
    expect(activeRange(text, 2, 8)).toEqual({ from: 0, to: 10 });
  });

  it('gère la première et la dernière ligne', () => {
    const text = 'alpha\nbeta';
    expect(activeRange(text, 0, 0)).toEqual({ from: 0, to: 5 });
    expect(activeRange(text, 10, 10)).toEqual({ from: 6, to: 10 });
  });
});

describe('escamotage du balisage', () => {
  it('masque les dièses d’un titre, espace compris', () => {
    // L'espace doit partir avec le dièse, sinon le titre rendu est indenté.
    expect(hidden('## Titre\n')).toEqual(['## ']);
  });

  it('masque les marqueurs d’emphase', () => {
    expect(hidden('du **gras** ici\n')).toEqual(['**', '**']);
    expect(hidden('du *italique* ici\n')).toEqual(['*', '*']);
  });

  it('masque les accents graves du code en ligne', () => {
    expect(hidden('appeler `code()` ici\n')).toEqual(['`', '`']);
  });

  it('laisse visibles les délimiteurs d’un bloc de code clôturé', () => {
    // Les masquer ferait disparaître la frontière du bloc et son langage.
    const text = '```bash\nls -l\n```\n';
    expect(hidden(text)).toEqual([]);
  });

  it('masque la cible d’un lien mais garde son libellé', () => {
    const text = 'voir [le site](https://exemple.fr) ici\n';
    // Lezer émet chaque délimiteur séparément : '[', ']', '(' puis ')'.
    expect(hidden(text)).toEqual(['[', ']', '(', 'https://exemple.fr', ')']);
  });

  it('masque le chevron d’une citation', () => {
    expect(hidden('> cité\n')).toEqual(['> ']);
  });

  it('ne masque rien sur la ligne où se trouve le curseur', () => {
    const text = '## Titre\ndu **gras**\n';
    // Curseur dans le titre : son source est visible, le reste est rendu.
    expect(hidden(text, 3, 3)).toEqual(['**', '**']);
    // Curseur dans le paragraphe : c'est l'inverse.
    expect(hidden(text, 12, 12)).toEqual(['## ']);
  });

  it('révèle toutes les lignes couvertes par une sélection', () => {
    const text = '## Titre\ndu **gras**\n';
    expect(hidden(text, 0, text.length)).toEqual([]);
  });
});

describe('styles', () => {
  it('attribue un niveau de titre à la ligne', () => {
    expect(lineClasses('# Un\n')).toEqual(['cm-lp-h1']);
    expect(lineClasses('### Trois\n')).toEqual(['cm-lp-h3']);
  });

  it('conserve le style de titre même sur la ligne active', () => {
    // Seul le balisage se révèle ; la taille du titre ne doit pas clignoter
    // quand le curseur entre dans la ligne.
    expect(lineClasses('# Un\n', 2, 2)).toEqual(['cm-lp-h1']);
  });

  it('style le gras, l’italique et le code', () => {
    expect(markClasses('**a** *b* `c`\n')).toEqual([
      'cm-lp-strong',
      'cm-lp-em',
      'cm-lp-code',
    ]);
  });

  it('style toutes les lignes d’un bloc de code', () => {
    const classes = lineClasses('```\nun\ndeux\n```\n');
    expect(classes.every((c) => c === 'cm-lp-fence')).toBe(true);
    expect(classes.length).toBeGreaterThanOrEqual(3);
  });
});

describe('tableaux', () => {
  const TABLE = '| a | b |\n| - | - |\n| 1 | 2 |\n';

  it('repère un tableau GFM à rendre', () => {
    const { tables } = analyse(TABLE, -1, -1);
    expect(tables).toHaveLength(1);
    expect(TABLE.slice(tables[0].from, tables[0].to)).toContain('| 1 | 2 |');
  });

  it('rend la main au source quand le curseur est dans le tableau', () => {
    expect(analyse(TABLE, 3, 3).tables).toEqual([]);
  });

  it('n’escamote rien à l’intérieur d’un tableau rendu', () => {
    // Le tableau est remplacé en bloc : décorer son contenu en plus n'aurait
    // aucun effet et risquerait des chevauchements.
    expect(hidden(TABLE)).toEqual([]);
  });
});

describe('robustesse', () => {
  it('accepte un document vide', () => {
    expect(analyse('', -1, -1)).toEqual({ hides: [], marks: [], lines: [], tables: [] });
  });

  it('ne produit jamais de plage inversée ni hors bornes', () => {
    const text = '# T\n**g** `c` [l](u)\n> q\n| a |\n| - |\n';
    const { hides, marks, lines, tables } = analyse(text, -1, -1);
    for (const r of [...hides, ...marks, ...tables]) {
      expect(r.from).toBeLessThanOrEqual(r.to);
      expect(r.from).toBeGreaterThanOrEqual(0);
      expect(r.to).toBeLessThanOrEqual(text.length);
    }
    for (const l of lines) {
      expect(l.pos).toBeGreaterThanOrEqual(0);
      expect(l.pos).toBeLessThanOrEqual(text.length);
    }
  });

  describe('ligne horizontale', () => {
    // Trois tirets et quatre tirets sont l'un comme l'autre une coupure ; c'est
    // la forme qu'un utilisateur écrit sans réfléchir.
    it.each(['---', '----', '***', '___'])('trace un trait pour « %s »', (rule) => {
      const text = `Avant\n\n${rule}\n\nAprès\n`;
      const { hides, lines } = analyse(text, -1, -1);

      const start = text.indexOf(rule);
      expect(lines).toContainEqual({ pos: start, cls: 'cm-lp-hr' });
      // Les tirets eux-mêmes disparaissent : sans cela le trait doublerait le
      // texte au lieu de le remplacer.
      expect(hides).toContainEqual({ from: start, to: start + rule.length });
    });

    /**
     * Le style de ligne est maintenu sur la ligne active pour les titres, mais
     * pas ici : un trait tracé au milieu barrerait le `---` en cours d'édition.
     */
    it('rend la source et efface le trait sur la ligne active', () => {
      const text = 'Avant\n\n---\n\nAprès\n';
      const start = text.indexOf('---');
      const { hides, lines } = analyse(text, start + 1, start + 1);

      expect(lines).not.toContainEqual({ pos: start, cls: 'cm-lp-hr' });
      expect(hides).not.toContainEqual({ from: start, to: start + 3 });
    });

    /**
     * `---` sous une ligne de texte n'est pas une coupure mais un titre de
     * niveau 2 en notation setext. Le confondre transformerait un titre en
     * trait et ferait disparaître son texte.
     */
    it('ne confond pas un titre setext avec une coupure', () => {
      const text = 'Mon titre\n---\n\nSuite\n';
      const { lines } = analyse(text, -1, -1);
      expect(lines.some((l) => l.cls === 'cm-lp-hr')).toBe(false);
    });
  });

  it('préserve le texte visible : rien d’utile n’est escamoté', () => {
    const text = '## Titre\n\nUn **mot** important.\n';
    const { hides } = analyse(text, -1, -1);
    let visible = text;
    for (const h of [...hides].sort((a, b) => b.from - a.from)) {
      visible = visible.slice(0, h.from) + visible.slice(h.to);
    }
    expect(visible).toContain('Titre');
    expect(visible).toContain('mot');
    expect(visible).toContain('important');
    expect(visible).not.toContain('#');
    expect(visible).not.toContain('*');
  });
});
