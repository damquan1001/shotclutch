"""Generate committed platform art/layout. Needs PySide6, Pillow >=12, ds_store 1.3.3, mac_alias 2.2.3."""
from pathlib import Path
import os,io,subprocess,sys
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtGui import QGuiApplication,QImage,QPainter,QFontDatabase
from PySide6.QtCore import Qt,QRectF
from PySide6.QtSvg import QSvgRenderer
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
app=QGuiApplication([])
font_id=QFontDatabase.addApplicationFont(str(ROOT/'assets/fonts/Inter-Regular.ttf'))
assert font_id >= 0, 'The retained Inter font must load for installer text.'
DIR=ROOT/'assets/app-icon'
SVG=DIR/'shotclutch.svg'
renderer=QSvgRenderer(str(SVG));assert renderer.isValid()
base=(DIR/'shotclutch.svg.attribution').read_text(encoding='utf-8')
def publish(path, data):
    staged=path.with_name(path.name+'.generation-tmp')
    staged.write_bytes(data)
    os.replace(staged,path)
def attribute(path, title=None):
    text=base.replace('asset: shotclutch.svg','asset: '+path.name)
    if title:
        text=text.replace('title: ShotClutch film-frame play mark','title: '+title)
        text=text.replace('source: original geometric artwork authored for ShotClutch M0','source: original minimal ShotClutch installation layout; no external artwork')
        text=text.replace('added: 2026-10-26','added: 2026-11-01')
    publish(path.with_name(path.name+'.attribution'),text.encode('utf-8'))
def render(size,path,mac=False):
    image=QImage(size,size,QImage.Format.Format_ARGB32);image.fill(Qt.GlobalColor.transparent)
    painter=QPainter(image)
    margin=size*0.098 if mac else 0
    renderer.render(painter,QRectF(margin,margin,size-2*margin,size-2*margin));painter.end()
    staged=path.with_name(path.name+'.generation-tmp')
    assert image.save(str(staged),'PNG')
    os.replace(staged,path);attribute(path)
render(1024,DIR/'shotclutch-1024.png')
render(512,DIR/'shotclutch-macos-512.png',True)
render(256,DIR/'shotclutch.png')
(DIR/'shotclutch-small.svg').write_bytes(SVG.read_bytes());attribute(DIR/'shotclutch-small.svg')
for size in [16,24,32,48,64,128,256,512]:
    p=DIR/f'hicolor/{size}x{size}/apps/com.shotclutch.editor.png';p.parent.mkdir(parents=True,exist_ok=True);render(size,p)
p=DIR/'hicolor/scalable/apps/com.shotclutch.editor.svg';p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(SVG.read_bytes());attribute(p)
image=Image.open(DIR/'shotclutch-1024.png')
buffer=io.BytesIO();image.save(buffer,format='ICO',sizes=[(s,s) for s in [16,20,24,32,40,48,64,128,256]])
publish(DIR/'shotclutch.ico',buffer.getvalue());attribute(DIR/'shotclutch.ico')
# Pillow's cross-platform ICNS writer embeds PNG representations; no macOS toolchain required.
buffer=io.BytesIO();image.save(buffer,format='ICNS');publish(DIR/'shotclutch.icns',buffer.getvalue());attribute(DIR/'shotclutch.icns')
p=ROOT/'apps/shotclutch-web/web/favicon.png';render(128,p)
bg=ROOT/'packaging/macos/dmg/background.svg'
bg.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="660" height="400" viewBox="0 0 660 400"><rect width="660" height="400" fill="#f1f3f5"/><text x="330" y="75" text-anchor="middle" font-family="Inter" font-size="32" fill="#171b23">ShotClutch</text><path d="M290 205h80m-20-20 20 20-20 20" fill="none" stroke="#14815c" stroke-width="6"/><text x="330" y="345" text-anchor="middle" font-family="Inter" font-size="18" fill="#171b23">Drag ShotClutch to Applications</text></svg>',encoding='utf-8');attribute(bg,'ShotClutch installation background')
# Finder draws black labels below both icons; the neutral light field keeps those legible.
pages=[]
for scale in [1,2]:
    img=QImage(660*scale,400*scale,QImage.Format.Format_ARGB32);img.fill(Qt.GlobalColor.transparent)
    painter=QPainter(img);QSvgRenderer(str(bg)).render(painter);painter.end()
    raw=img.convertToFormat(QImage.Format.Format_RGBA8888)
    pages.append(Image.frombytes('RGBA',(660*scale,400*scale),bytes(raw.constBits())).convert('RGB'))
pages[1].encoderinfo={'dpi':(144,144)}
buffer=io.BytesIO();pages[0].save(buffer,format='TIFF',save_all=True,append_images=pages[1:],compression='tiff_adobe_deflate',dpi=(72,72))
publish(bg.with_suffix('.tiff'),buffer.getvalue());attribute(bg.with_suffix('.tiff'),'ShotClutch installation background')
subprocess.run([sys.executable,str(ROOT/'packaging/macos/dmg/generate.py')],check=True)
print('Generated canonical ShotClutch platform icons and installer background.')
