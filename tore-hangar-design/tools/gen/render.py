import json, os, sys, asyncio
from playwright.async_api import async_playwright
OUT=sys.argv[1]; names=sys.argv[2:]
t=json.load(open(f"{OUT}/project/tokens.json"))
css=":root{"+"".join(f"--{c['name']}:{c['value']};" for c in t['color']['tokens'])
for fam in ['spacing','radius','shadow']:
    css+="".join(f"--{c['name']}:{c['value']};" for c in t[fam]['tokens'])
css+="".join(f"--font-{k}:{v};" for k,v in t['type']['families'].items())+"}"
bcss=open(f"{OUT}/project/components/bundle.css").read()
async def main():
    async with async_playwright() as p:
        b=await p.chromium.launch()
        for n in names:
            html=open(f"{OUT}/project/components/{n}/preview.html").read()
            html=html.replace("<head>",f'<head><style>{css}{bcss}</style>',1).replace("<html>",'<html data-theme="gunmetal">',1)
            pg=await b.new_page(viewport={"width":1300 if n=="Workspace" else 980,"height":820})
            await pg.set_content(html); await pg.wait_for_timeout(1200)
            await pg.screenshot(path=f"shots/{n}.png",full_page=True)
        await b.close()
asyncio.run(main())
