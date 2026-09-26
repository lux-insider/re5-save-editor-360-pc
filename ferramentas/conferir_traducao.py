"""Confere as traduções da História (ferramentas/trad/NN.txt): mesma
quantidade de páginas do original. Formato: título, linha "#", páginas
separadas por linhas "@@". Com --gravar, junta tudo em src/dados/historia.json
(campos titulo_pt e paginas_pt)."""
import json, os, sys
P = 'src/dados/historia.json'
d = json.load(open(P))
ok = falta = 0
for i, t in enumerate(d['textos']):
    f = f'ferramentas/trad/{i:02d}.txt'
    if not os.path.exists(f):
        falta += 1
        continue
    s = open(f).read().strip()
    titulo, corpo = s.split('\n#\n', 1)
    pags = [p.strip() for p in corpo.split('\n@@\n')]
    if len(pags) != len(t['paginas']):
        print(f'{i}: {len(pags)} páginas, original {len(t["paginas"])}')
        continue
    ok += 1
    t['titulo_pt'], t['paginas_pt'] = titulo.strip(), pags
print(f'{ok} ok, {falta} faltando')
if '--gravar' in sys.argv:
    json.dump(d, open(P, 'w'), ensure_ascii=False, separators=(',', ':'))
