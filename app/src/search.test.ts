import { describe, expect, it } from 'vitest';
import { splitChapters } from './structure';
import { findAll, groupByChapter, nextMatch, replaceAll } from './search';

const DOC = '# Alpha\nchat noir\n# Beta\nCHAT blanc\nchat gris\n';
const CHAPTERS = splitChapters(DOC);

describe('recherche', () => {
  it('porte sur le document entier, pas sur le chapitre affiché', () => {
    const matches = findAll(DOC, 'chat', CHAPTERS, false);
    expect(matches).toHaveLength(3);
    expect(matches.map((m) => m.chapterIndex)).toEqual([0, 1, 1]);
  });

  it('respecte réellement la casse', () => {
    expect(findAll(DOC, 'chat', CHAPTERS, true)).toHaveLength(2);
    expect(findAll(DOC, 'CHAT', CHAPTERS, true)).toHaveLength(1);
  });

  it('traite le terme comme du texte littéral, jamais comme une regex', () => {
    // Le bug de la v1 : `(` levait une exception.
    const doc = 'valeur (a+b) puis [x] et a.b';
    const chapters = splitChapters(doc);
    expect(() => findAll(doc, '(', chapters, false)).not.toThrow();
    expect(findAll(doc, '(a+b)', chapters, false)).toHaveLength(1);
    expect(findAll(doc, '[x]', chapters, false)).toHaveLength(1);
    // `.` ne doit pas se comporter comme un joker.
    expect(findAll(doc, 'a.b', chapters, false)).toHaveLength(1);
    expect(findAll(doc, 'a.', chapters, false)).toHaveLength(1);
  });

  it('ne boucle pas sur des occurrences qui se chevauchent', () => {
    const doc = 'aaaa';
    expect(findAll(doc, 'aa', splitChapters(doc), false)).toHaveLength(2);
  });

  it('rend une liste vide pour un terme vide', () => {
    expect(findAll(DOC, '', CHAPTERS, false)).toEqual([]);
  });

  it('fournit un extrait de contexte sur une seule ligne', () => {
    const [first] = findAll(DOC, 'chat', CHAPTERS, false);
    expect(first.excerpt).toContain('chat');
    expect(first.excerpt).not.toContain('\n');
  });
});

describe('regroupement par chapitre', () => {
  it('ordonne les groupes et nomme les chapitres', () => {
    const groups = groupByChapter(findAll(DOC, 'chat', CHAPTERS, false), CHAPTERS);
    expect(groups.map((g) => [g.chapterTitle, g.matches.length])).toEqual([
      ['Alpha', 1],
      ['Beta', 2],
    ]);
  });
});

describe('occurrence suivante', () => {
  const matches = findAll(DOC, 'chat', CHAPTERS, false);

  it('avance vers la suivante', () => {
    expect(nextMatch(matches, -1)).toBe(matches[0]);
    expect(nextMatch(matches, matches[0].from)).toBe(matches[1]);
  });

  it('repart au début une fois la fin atteinte', () => {
    expect(nextMatch(matches, DOC.length)).toBe(matches[0]);
  });

  it('rend null s’il n’y a rien à parcourir', () => {
    expect(nextMatch([], 0)).toBeNull();
  });
});

describe('remplacer tout', () => {
  it('remplace uniquement les occurrences trouvées', () => {
    const matches = findAll(DOC, 'chat', CHAPTERS, true);
    const result = replaceAll(DOC, matches, 'chien');
    expect(result.count).toBe(2);
    // La variante en majuscules n'a pas été touchée : la casse est respectée,
    // contrairement au `String.replace` de la v1.
    expect(result.text).toContain('CHAT blanc');
    expect(result.text).toContain('chien noir');
    expect(result.text).toContain('chien gris');
  });

  it('annonce le nombre de chapitres touchés', () => {
    const matches = findAll(DOC, 'chat', CHAPTERS, false);
    expect(replaceAll(DOC, matches, 'chien').chaptersTouched).toBe(2);
  });

  it('ne modifie rien sans occurrence', () => {
    expect(replaceAll(DOC, [], 'chien')).toEqual({ text: DOC, count: 0, chaptersTouched: 0 });
  });

  it('gère un remplacement par une chaîne vide', () => {
    const doc = 'abcabc';
    const matches = findAll(doc, 'b', splitChapters(doc), false);
    expect(replaceAll(doc, matches, '').text).toBe('acac');
  });

  it('gère un remplacement plus long que le terme', () => {
    const doc = 'a-a-a';
    const matches = findAll(doc, '-', splitChapters(doc), false);
    expect(replaceAll(doc, matches, '===').text).toBe('a===a===a');
  });
});
