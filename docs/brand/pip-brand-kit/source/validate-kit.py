"""Validate existing Pip assets, package PNG favicon sizes as ICO, and hash the kit."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import xml.etree.ElementTree as ET
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SVG_NS = '{http://www.w3.org/2000/svg}'

def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def inspect_png(path: Path) -> dict:
    with Image.open(path) as im:
        assert im.mode == 'RGBA', (path.name, im.mode)
        a = im.getchannel('A')
        minimum, maximum = a.getextrema()
        # Tiny antialiased strokes need not contain a fully opaque pixel.
        assert minimum == 0 and maximum >= 240, (path.name, minimum, maximum)
        visible = a.point(lambda v: 255 if v >= 8 else 0).getbbox()
        assert visible is not None
        x0, y0, x1, y1 = visible
        assert 0 < x0 < x1 < im.width and 0 < y0 < y1 < im.height, (path.name, visible)
        corners = [a.getpixel(xy) for xy in [(0,0),(im.width-1,0),(0,im.height-1),(im.width-1,im.height-1)]]
        assert max(corners) <= 1, (path.name, corners)
        return dict(file=path.relative_to(ROOT).as_posix(), width=im.width, height=im.height,
                    mode=im.mode, alphaRange=[minimum, maximum], rawAlphaBounds=a.getbbox(),
                    visibleBoundsAtAlpha8=visible, cornerAlpha=corners,
                    fullyTransparentPixelFraction=round(a.histogram()[0]/(im.width*im.height),6))

def make_ico(theme: str) -> dict:
    sizes=[16,32,48]
    images=[(ROOT/f'favicons/plenipo-favicon-on-{theme}-{size}.png').read_bytes() for size in sizes]
    offset=6+16*len(images)
    header=struct.pack('<HHH',0,1,len(images))
    entries=[]
    for size,data in zip(sizes,images):
        entries.append(struct.pack('<BBBBHHII',size,size,0,0,1,32,len(data),offset))
        offset+=len(data)
    target=ROOT/f'favicons/plenipo-favicon-on-{theme}.ico'
    target.write_bytes(header+b''.join(entries)+b''.join(images))
    with Image.open(target) as ico:
        actual=sorted(ico.ico.sizes())
        assert actual == [(16,16),(32,32),(48,48)], actual
        for size in sizes:
            assert ico.ico.getimage((size,size)).convert('RGBA').tobytes() == Image.open(ROOT/f'favicons/plenipo-favicon-on-{theme}-{size}.png').tobytes()
    return dict(file=target.relative_to(ROOT).as_posix(), sizes=actual, exactPngFrames=True)

def validate_svgs() -> list[dict]:
    output=[]
    master=ET.parse(ROOT/'source/approved-wordmark.svg').getroot()
    master_paths=master.findall('.//'+SVG_NS+'path')
    for file in sorted((ROOT/'logos').glob('*.svg'))+sorted((ROOT/'favicons').glob('*.svg')):
        doc=ET.parse(file).getroot()
        allowed={'svg','title','desc','g','path','image'}
        assert all(e.tag.removeprefix(SVG_NS) in allowed for e in doc.iter()), file.name
        assert all(not k.lower().startswith('on') for e in doc.iter() for k in e.attrib), file.name
        assert all(v.startswith('data:image/png;base64,') for e in doc.iter() for k,v in e.attrib.items() if k.endswith('href')),file.name
        paths=doc.findall('.//'+SVG_NS+'path')
        images=doc.findall('.//'+SVG_NS+'image')
        horizontal='horizontal' in file.name
        expected=master_paths if horizontal else master_paths[:3]
        assert [e.get('d') for e in paths]==[e.get('d') for e in expected],file.name
        assert len(images)==(0 if 'favicon' in file.name else 1),file.name
        theme='dark' if 'on-dark' in file.name else 'light'
        primary='#5b97ff' if theme=='dark' else '#2463eb'
        assert paths[0].get('stroke')==primary
        if horizontal:
            assert paths[-1].get('fill')==primary
            assert all(e.get('fill')==('#f3f7ff' if theme=='dark' else '#0b1833') for e in paths[3:-1])
        output.append(dict(file=file.relative_to(ROOT).as_posix(),viewBox=doc.get('viewBox'),
                           pathCount=len(paths),embeddedRasterCount=len(images),externalDependencies=False,
                           nativePathGeometryPreserved=True, outlinedWordmark=horizontal))
    assert len(output)==6
    return output

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--archive',type=Path)
    args=parser.parse_args()
    spec=json.loads((ROOT/'source/prompts.json').read_text(encoding='utf-8'))
    poses=sorted((ROOT/'mascots').glob('*.png'))
    assert len(poses)==15 and len(spec['assets'])==15
    assert len({sha256(p) for p in poses})==15
    pngs=poses+sorted((ROOT/'logos').glob('*.png'))+sorted((ROOT/'favicons').glob('*.png'))
    assert len(pngs)==27
    image_checks=[inspect_png(p) for p in pngs]
    assert all((x['width'],x['height'])==(1254,1254) for x in image_checks if x['file'].startswith('mascots/'))
    assert all((x['width'],x['height'])==(3000,1194) for x in image_checks if 'horizontal' in x['file'])
    assert all((x['width'],x['height'])==(2000,2000) for x in image_checks if 'square' in x['file'])
    assert all(x['alphaRange']==[0,255] for x in image_checks if x['file'].startswith(('mascots/','logos/')))
    ico_checks=[make_ico(t) for t in ['light','dark']]
    svg_checks=validate_svgs()
    receipt={
        'checkedAt':datetime.now(timezone.utc).isoformat(),
        'productionCounts':{'mascotPng':15,'logoPng':4,'logoHybridSvg':4,'faviconPng':8,'faviconVectorSvg':2,'faviconIco':2},
        'pngChecks':image_checks,'svgChecks':svg_checks,'icoChecks':ico_checks,
        'visualQa':{
            'method':'Actual saved preview PNGs opened with view_image, composed from the exact saved production files.',
            'files':['previews/pip-all-15-on-light.png','previews/pip-all-15-on-dark.png','previews/plenipo-logo-family.png','previews/plenipo-favicon-sizes.png'],
            'observations':'15 distinct consistent full-body Pip activities; no clipped antennae/feet/props; readable light/dark silhouettes; complete Plenipo wordmark; large initial P and matching blue o; Pip seated on n and square P; no attribution; actual 16/32/48 px favicon review.',
            'alphaNote':'Original generated alpha preserved, including negligible alpha-1 residual pixels in pose 01. No opaque matte or visible checkerboard. Visible bounds are reported at alpha >= 8.'
        },
        'scope':{'websiteChanged':False,'appCodeChanged':False,'deployed':False,'runtimeResourcesCreated':False}
    }
    # These earlier website review files are outside the kit and must remain intact.
    preserved={
        ROOT.parents[1]/'typography/refined/website.html':'c05954d4905acb6f6a55261245b3558d195fc49c851414c289e66d98daacd22d',
        ROOT.parents[1]/'typography/refined/refined-website.png':'78cfb64b703a42f35ff4c8d64907232f241fc417dc8584760bba7fb620476034'
    }
    receipt['preservedWebsiteReview']=[]
    for file,expected in preserved.items():
        if file.exists():
            actual=sha256(file)
            assert actual==expected,(file.name,actual)
            receipt['preservedWebsiteReview'].append({'file':file.name,'sha256':actual,'unchanged':True})
    (ROOT/'validation.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    files=[]
    for file in sorted(ROOT.rglob('*')):
        if file.is_file() and file.name!='manifest.json':
            record={'file':file.relative_to(ROOT).as_posix(),'bytes':file.stat().st_size,'sha256':sha256(file)}
            if file.suffix=='.png':
                with Image.open(file) as im:record.update(width=im.width,height=im.height,mode=im.mode)
            files.append(record)
    manifest={'name':'Plenipo + Pip Brand Kit','character':'Pip','version':'1.0','generatedWith':'Built-in image_gen.imagegen',
              'productionFolders':['mascots','logos','favicons'],'previewFolders':['previews'],
              'note':'Production artwork is transparent. Preview boards and original source reference may be opaque. Mascot-bearing SVGs embed raster Pip.',
              'files':files}
    (ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    report={'productionChecks':'passed','pngCount':len(pngs),'svgCount':len(svg_checks),'icoCount':len(ico_checks),'manifestFiles':len(files)}
    if args.archive:
        target=args.archive.resolve()
        assert not target.is_relative_to(ROOT), 'Place archive outside the kit folder.'
        with zipfile.ZipFile(target,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as archive:
            for file in sorted(ROOT.rglob('*')):
                if file.is_file():archive.write(file,Path('Plenipo-Pip-Brand-Kit')/file.relative_to(ROOT))
        with zipfile.ZipFile(target) as archive:
            assert archive.testzip() is None
            assert len(archive.namelist())==len(files)+1
            for entry in files:
                data=archive.read('Plenipo-Pip-Brand-Kit/'+entry['file'])
                assert hashlib.sha256(data).hexdigest()==entry['sha256'],entry['file']
        report.update(archive=str(target),archiveBytes=target.stat().st_size,archiveSha256=sha256(target),archiveIntegrity='passed')
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
