"""Mostra textos da História (páginas separadas por @@) para traduzir."""
import json, sys
t = json.load(open('src/dados/historia.json'))['textos']
for i in map(int, sys.argv[1:]):
    print(f'##### {i} | {t[i]["titulo"]}')
    print('\n@@\n'.join(t[i]['paginas']))
