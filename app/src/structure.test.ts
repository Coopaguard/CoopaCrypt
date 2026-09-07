import { describe, expect, it } from 'vitest';
import { chapterAt, findHeadings, replaceChapter, splitChapters } from './structure';

/** Raccourci de lecture : liste des titres de chapitres. */
const titles = (text: string) => splitChapters(text).map((c) => c.title);

/** Tranche effectivement affichée pour un chapitre donné. */
const slice = (text: string, index: number) => {
  const c = splitChapters(text)[index];
  return text.slice(c.from, c.to);
};

describe('découpage en chapitres', () => {
  it('garantit toujours au moins un chapitre', () => {
    expect(titles('')).toEqual(['Document']);
    expect(titles('du texte sans titre')).toEqual(['Document']);
  });

  it('ouvre un chapitre par titre de niveau 1', () => {
    const doc = '# Comptes\nligne\n\n# Serveurs\nautre\n';
    expect(titles(doc)).toEqual(['Comptes', 'Serveurs']);
    expect(slice(doc, 0)).toBe('# Comptes\nligne\n\n');
    expect(slice(doc, 1)).toBe('# Serveurs\nautre\n');
  });

  it('place le texte précédant le premier titre dans un chapitre implicite', () => {
    const doc = 'préambule\n\n# Comptes\nligne\n';
    const chapters = splitChapters(doc);
    expect(chapters.map((c) => c.title)).toEqual(['Introduction', 'Comptes']);
    expect(chapters[0].implicit).toBe(true);
    expect(chapters[1].implicit).toBe(false);
  });

  it('ne découpe pas sur un dièse situé dans un bloc de code', () => {
    // Le piège signalé par UI.md §2.2 : un script shell commenté.
    const doc = ['# Notes', '', '```bash', '# ceci est un commentaire', 'ls -l', '```', '', '# Vrai chapitre', ''].join(
      '\n',
    );
    expect(titles(doc)).toEqual(['Notes', 'Vrai chapitre']);
  });

  it('ignore aussi les dièses dans un bloc clôturé par des tildes', () => {
    const doc = ['# Notes', '~~~', '# pas un titre', '~~~', '# Suite'].join('\n');
    expect(titles(doc)).toEqual(['Notes', 'Suite']);
  });

  it('accepte les titres homonymes, distingués par leur position', () => {
    const chapters = splitChapters('# Divers\na\n# Divers\nb\n');
    expect(chapters.map((c) => c.title)).toEqual(['Divers', 'Divers']);
    expect(chapters.map((c) => c.index)).toEqual([0, 1]);
    expect(chapters[0].from).not.toBe(chapters[1].from);
  });

  it('nomme les titres vides plutôt que d’afficher du vide', () => {
    expect(titles('#\ncontenu\n')).toEqual(['(sans titre)']);
  });

  it('retire les dièses de fermeture d’un titre fermé', () => {
    expect(titles('# Comptes #\n')).toEqual(['Comptes']);
  });
});

describe('sous-parties', () => {
  it('rattache chaque sous-partie à son chapitre', () => {
    const doc = '# A\n## A1\n## A2\n# B\n## B1\n### B1a\n';
    const [a, b] = splitChapters(doc);
    expect(a.subsections.map((s) => s.title)).toEqual(['A1', 'A2']);
    expect(b.subsections.map((s) => [s.title, s.level])).toEqual([
      ['B1', 2],
      ['B1a', 3],
    ]);
  });

  it('ne laisse pas les sous-parties découper l’affichage', () => {
    const doc = '# A\ntexte\n## A1\nsuite\n';
    expect(splitChapters(doc)).toHaveLength(1);
    expect(slice(doc, 0)).toBe(doc);
  });
});

describe('navigation par décalage', () => {
  const doc = '# A\naaa\n# B\nbbb\n';

  it('retrouve le chapitre contenant un décalage', () => {
    const chapters = splitChapters(doc);
    expect(chapterAt(chapters, doc.indexOf('aaa')).title).toBe('A');
    expect(chapterAt(chapters, doc.indexOf('bbb')).title).toBe('B');
  });

  it('rattache un décalage en fin de document au dernier chapitre', () => {
    const chapters = splitChapters(doc);
    expect(chapterAt(chapters, doc.length).title).toBe('B');
  });
});

describe('édition par tranche', () => {
  it('remplace une tranche sans toucher au reste du document', () => {
    const doc = '# A\naaa\n# B\nbbb\n';
    const chapters = splitChapters(doc);
    expect(replaceChapter(doc, chapters[0], '# A\nmodifié\n')).toBe('# A\nmodifié\n# B\nbbb\n');
    expect(replaceChapter(doc, chapters[1], '# B\nautre\n')).toBe('# A\naaa\n# B\nautre\n');
  });

  it('scinde un chapitre quand un titre est ajouté', () => {
    const doc = '# A\naaa\n';
    const modifie = replaceChapter(doc, splitChapters(doc)[0], '# A\naaa\n# Nouveau\n');
    expect(titles(modifie)).toEqual(['A', 'Nouveau']);
  });

  it('fusionne deux chapitres quand un titre est supprimé', () => {
    const doc = '# A\naaa\n# B\nbbb\n';
    const chapters = splitChapters(doc);
    const modifie = replaceChapter(doc, chapters[1], 'bbb\n');
    expect(titles(modifie)).toEqual(['A']);
  });
});

describe('intégrité du contenu', () => {
  it('couvre le document entier sans trou ni chevauchement', () => {
    const doc = 'tête\n# A\naaa\n# B\nbbb';
    const chapters = splitChapters(doc);
    expect(chapters[0].from).toBe(0);
    expect(chapters[chapters.length - 1].to).toBe(doc.length);
    for (let i = 1; i < chapters.length; i++) {
      expect(chapters[i].from).toBe(chapters[i - 1].to);
    }
    expect(chapters.map((c) => doc.slice(c.from, c.to)).join('')).toBe(doc);
  });

  it('préserve les caractères non-ASCII et les CRLF', () => {
    const doc = '# Café\r\nmot : café́ 🔐\r\n# Fin\r\n';
    const chapters = splitChapters(doc);
    expect(chapters.map((c) => doc.slice(c.from, c.to)).join('')).toBe(doc);
    expect(chapters[0].title).toBe('Café');
  });
});

describe('findHeadings', () => {
  it('rend les titres dans l’ordre du document', () => {
    const headings = findHeadings('## B\n# A\n### C\n');
    expect(headings.map((h) => h.title)).toEqual(['B', 'A', 'C']);
    expect(headings.map((h) => h.level)).toEqual([2, 1, 3]);
  });
});
