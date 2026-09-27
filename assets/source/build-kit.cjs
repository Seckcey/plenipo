/* Native SVG logo composition and diagnostic contact sheets.
 * Generated mascot PNG bytes and their alpha are never modified.
 * Run: node source/build-kit.cjs [optional path to sharp]
 */
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require(process.argv[2] || 'sharp');
const root = path.resolve(__dirname, '..');
const palette = {
  light: { ink:'#0b1833', primary:'#2463eb', middle:'#5898f3', inner:'#123c88', background:'#f5f7fb', card:'#ffffff', muted:'#526078', line:'#dce3ef' },
  dark: { ink:'#f3f7ff', primary:'#5b97ff', middle:'#8cbcff', inner:'#d8e8ff', background:'#081226', card:'#111f36', muted:'#a9b8d2', line:'#2a3c57' }
};
const esc = value => String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const imageTag = (uri,x,y,w,h) => `<image x="${x}" y="${y}" width="${w}" height="${h}" href="${uri}"/>`;
const label = (s,x,y,size,color,weight=400) => `<text x="${x}" y="${y}" font-family="Arial, sans-serif" font-size="${size}" font-weight="${weight}" fill="${color}">${esc(s)}</text>`;
const svg = (w,h,title,body,description='') => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(title)}"><title>${esc(title)}</title><desc>${esc(description)}</desc>${body}</svg>`;
const pngUri = async file => `data:image/png;base64,${(await fs.readFile(file)).toString('base64')}`;
async function writeSvgPng(relative,source,width){
  await fs.writeFile(path.join(root,`${relative}.svg`),source);
  await sharp(Buffer.from(source)).resize({width}).png().toFile(path.join(root,`${relative}.png`));
}
const iconPaths = p => `<g fill="none" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"><path d="M19 15H56A26 26 0 0 1 56 67H43A24 24 0 0 0 19 91" stroke="${p.primary}"/><path d="M19 30H54A12 12 0 0 1 54 54H42A10 10 0 0 0 32 64V91" stroke="${p.middle}"/><path d="M19 43H44V91" stroke="${p.inner}"/></g>`;
async function main(){
  for (const dir of ['logos','favicons','previews']) await fs.mkdir(path.join(root,dir),{recursive:true});
  const spec = JSON.parse(await fs.readFile(path.join(root,'source/prompts.json'),'utf8'));
  const master = await fs.readFile(path.join(root,'source/approved-wordmark.svg'),'utf8');
  const artwork = master.slice(master.indexOf('<g id="capital-p"'),master.lastIndexOf('</svg>'));
  const pip = await pngUri(path.join(root,'mascots/pip-01-coding.png'));
  for (const [theme,p] of Object.entries(palette)){
    const recolored = artwork.replaceAll('#0b1833',p.ink).replaceAll('#2463eb',p.primary).replaceAll('#5898f3',p.middle).replaceAll('#123c88',p.inner);
    const horizontal = svg(749,298,`Plenipo with Pip - for ${theme} backgrounds`,recolored+imageTag(pip,329,32,132,132),'The large blue P icon is the initial capital P, followed by outlined lenipo in Sentient Medium. The final o matches the blue P. Pip sits on the n using his laptop. Transparent background; no attribution.');
    await writeSvgPng(`logos/plenipo-horizontal-on-${theme}`,horizontal,3000);
    const square = svg(800,800,`Plenipo square with Pip - for ${theme} backgrounds`,`<g transform="translate(94.75 196.75) scale(5.5)">${iconPaths(p)}</g>`+imageTag(pip,273,28,325,325),'Text-free square logo: Pip coding on the enlarged blue P icon. Transparent background.');
    await writeSvgPng(`logos/plenipo-square-on-${theme}`,square,2000);
    const favicon = svg(100,100,`Plenipo P - for ${theme} backgrounds`,`<g transform="translate(-0.5 -3)">${iconPaths(p)}</g>`,'The three-rail P symbol, with no mascot or wordmark, optimized for small sizes. Transparent background.');
    await fs.writeFile(path.join(root,`favicons/plenipo-favicon-on-${theme}.svg`),favicon);
    for (const size of [16,32,48,256]) await sharp(Buffer.from(favicon)).resize(size,size).png().toFile(path.join(root,`favicons/plenipo-favicon-on-${theme}-${size}.png`));

    const width=1920,height=1500,margin=48,gap=20,cellW=(width-margin*2-gap*4)/5,cellH=400;
    let sheet=`<rect width="${width}" height="${height}" fill="${p.background}"/>`;
    sheet+=label('Meet Pip.',margin,83,54,p.ink,700)+label('15 poses for the Plenipo brand',margin,129,24,p.muted);
    sheet+=label(`PREVIEW ON ${theme.toUpperCase()}  /  INDIVIDUAL FILES HAVE TRANSPARENT BACKGROUNDS`,margin,166,14,p.muted,700);
    for (let i=0;i<spec.assets.length;i++){
      const item=spec.assets[i],x=margin+(i%5)*(cellW+gap),y=195+Math.floor(i/5)*(cellH+gap);
      sheet+=`<rect x="${x}" y="${y}" width="${cellW}" height="${cellH}" rx="20" fill="${p.card}" stroke="${p.line}"/>`;
      sheet+=imageTag(await pngUri(path.join(root,item.file)),x+(cellW-305)/2,y+14,305,305);
      sheet+=label(String(i+1).padStart(2,'0'),x+23,y+365,16,p.primary,700)+label(item.name,x+60,y+365,20,p.ink,700);
    }
    sheet+=label('Plenipo  /  Pip character collection',margin,1467,15,p.muted);
    await sharp(Buffer.from(svg(width,height,`Pip contact sheet on ${theme}`,sheet))).png().toFile(path.join(root,`previews/pip-all-15-on-${theme}.png`));
  }
  let logoBoard = `<rect width="1800" height="1420" fill="#edf1f8"/>`+label('Plenipo + Pip',54,82,46,palette.light.ink,700)+label('Finished logo family  /  Transparent PNG + self-contained SVG',54,126,22,palette.light.muted);
  for (const [index,theme] of ['light','dark'].entries()){
    const p=palette[theme],x=48+index*864;
    logoBoard+=`<rect x="${x}" y="168" width="840" height="1198" rx="22" fill="${p.background}"/>`;
    logoBoard+=label(`FOR ${theme.toUpperCase()} BACKGROUNDS`,x+34,219,16,p.muted,700);
    logoBoard+=imageTag(await pngUri(path.join(root,`logos/plenipo-horizontal-on-${theme}.png`)),x+22,282,796,317);
    logoBoard+=`<line x1="${x+34}" y1="657" x2="${x+806}" y2="657" stroke="${p.line}"/>`;
    logoBoard+=imageTag(await pngUri(path.join(root,`logos/plenipo-square-on-${theme}.png`)),x+155,725,530,530);
    logoBoard+=label('TEXT-FREE SQUARE',x+34,1316,15,p.muted,700);
  }
  await sharp(Buffer.from(svg(1800,1420,'Plenipo logo family preview',logoBoard))).png().toFile(path.join(root,'previews/plenipo-logo-family.png'));

  let faviconBoard=`<rect width="1440" height="650" fill="#edf1f8"/>`+label('The P, at browser size',40,70,38,palette.light.ink,700)+label('Actual 16 / 32 / 48 px exports, with a 4x diagnostic view underneath',40,109,18,palette.light.muted);
  for (const [index,theme] of ['light','dark'].entries()){
    const p=palette[theme],x=32+index*704;
    faviconBoard+=`<rect x="${x}" y="150" width="672" height="458" rx="18" fill="${p.background}"/>`+label(theme.toUpperCase(),x+28,198,16,p.muted,700);
    for(const [i,size] of [16,32,48].entries()){
      const left=x+70+i*200,uri=await pngUri(path.join(root,`favicons/plenipo-favicon-on-${theme}-${size}.png`));
      faviconBoard+=imageTag(uri,left+80-size/2,239,size,size)+label(`${size} px`,left+54,316,17,p.ink,700);
      const zoom=await sharp(path.join(root,`favicons/plenipo-favicon-on-${theme}-${size}.png`)).resize(size*4,size*4,{kernel:'nearest'}).png().toBuffer();
      faviconBoard+=imageTag(`data:image/png;base64,${zoom.toString('base64')}`,left+80-size*2,352,size*4,size*4);
    }
  }
  await sharp(Buffer.from(svg(1440,650,'Plenipo favicon readability preview',faviconBoard))).png().toFile(path.join(root,'previews/plenipo-favicon-sizes.png'));
  console.log('Built 4 transparent logo SVG/PNG pairs, 2 favicon SVGs with 8 PNG sizes, and 4 preview sheets. Mascot source PNGs preserved.');
}
main().catch(error=>{console.error(error);process.exitCode=1;});
