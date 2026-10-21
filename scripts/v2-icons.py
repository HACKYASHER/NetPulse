"""Render NetPulse's own geometric pulse mark for Windows resources."""
from pathlib import Path
from PIL import Image, ImageDraw
root=Path(__file__).resolve().parents[1]/'apps/desktop-v2/src-tauri/icons'
root.mkdir(parents=True,exist_ok=True)
image=Image.new('RGBA',(256,256),(0,0,0,0));draw=ImageDraw.Draw(image)
draw.rounded_rectangle((3,3,253,253),radius=58,fill='#8B7CF8')
draw.line([(42,136),(77,136),(99,78),(128,183),(153,115),(169,136),(214,136)],fill='#F5F6FA',width=13,joint='curve')
image.save(root/'icon.png');image.save(root/'icon.ico',sizes=[(16,16),(32,32),(48,48),(64,64),(128,128),(256,256)])
(root/'icon.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><rect x="3" y="3" width="250" height="250" rx="58" fill="#8B7CF8"/><path d="M42 136H77L99 78 128 183 153 115 169 136H214" fill="none" stroke="#F5F6FA" stroke-width="13" stroke-linejoin="round"/></svg>',encoding='utf8')
