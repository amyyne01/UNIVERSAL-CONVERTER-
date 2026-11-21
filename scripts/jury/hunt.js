// usage: node hunt.js dir  -> prints per file measurable defects
const fs=require('fs'),path=require('path');const dir=process.argv[2];
const hex=h=>{h=h.replace('#','');return [0,2,4,6].map(i=>parseInt(h.slice(i,i+2)||'ff',16))};
const lum=([r,g,b])=>{const f=c=>{c/=255;return c<=0.03928?c/12.92:Math.pow((c+0.055)/1.055,2.4)};return 0.2126*f(r)+0.7152*f(g)+0.0722*f(b)};
const blend=(fg,bg)=>{const a=fg[3]/255;return [0,1,2].map(i=>Math.round(fg[i]*a+bg[i]*(1-a))).concat(255)};
const cr=(a,b)=>{const x=lum(a),y=lum(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05)};
const inside=(r,R)=>r[0]>=R[0]-.5&&r[1]>=R[1]-.5&&r[0]+r[2]<=R[0]+R[2]+.5&&r[1]+r[3]<=R[1]+R[3]+.5;
const inter=(a,b)=>{const w=Math.min(a[0]+a[2],b[0]+b[2])-Math.max(a[0],b[0]),h=Math.min(a[1]+a[3],b[1]+b[3])-Math.max(a[1],b[1]);return w>1&&h>1?w*h:0};
for(const f of fs.readdirSync(dir).filter(f=>f.endsWith('.json')&&(!process.argv[3]||f.includes(process.argv[3])))){
 const d=JSON.parse(fs.readFileSync(path.join(dir,f)));const [vw,vh]=d.viewport;const E=d.elements.filter(e=>e.opacity>0.01&&!e.inert);
 const out=[];
 // background: painted elements in order; for each text find last opaque-ish painted element (before it in list) containing it
 E.forEach((e,i)=>{
  if(e.text&&e.color&&!E.some(c=>c!==e&&c.path.startsWith(e.path+"/"))){
   let bg=[0,0,0,255];const stack=[];
   for(let j=0;j<i;j++){const p=E[j];const c=hex(p.fill);if(c[3]>0&&inside(e.rect,p.rect))stack.push(c)}
   bg=stack.reduce((acc,c)=>blend(c,acc),[0,0,0,255]);
   const fg=blend(hex(e.color),bg);const ratio=cr(fg,bg);
   const small=e.rect[3]<18; if(ratio<(small?4.5:3)) out.push(`LOWCONTRAST ${ratio.toFixed(2)} "${e.text.slice(0,40)}" ${e.color} on ${'#'+bg.slice(0,3).map(x=>x.toString(16).padStart(2,'0')).join('')} h${e.rect[3]}`);
   if(e.rect[0]<-1||e.rect[0]+e.rect[2]>vw+1) out.push(`OFFSCREEN "${e.text.slice(0,40)}" ${e.rect.map(Math.round)}`);
   if(/\{|\}|::|TODO|lorem|_[A-Z]{3,}|^[A-Z_]{6,}$/.test(e.text)) out.push(`RAWTEXT "${e.text}"`);
  }
  if(['button','link','checkbox','switch','tab','textbox','radio','slider','menuitem','combobox'].includes(e.role)||e.focusable){
   const [x,y,w,h]=e.rect; if((w<24||h<24)&&w>0) out.push(`SMALLHIT ${e.role} ${Math.round(w)}x${Math.round(h)} "${(e.label||e.key||e.path.split('/').slice(-2).join('/')).slice(0,40)}"`);
  }
 });
 // overlapping texts
 const T=E.filter(e=>e.text);for(let a=0;a<T.length;a++)for(let b=a+1;b<T.length;b++){const o=inter(T[a].rect,T[b].rect);if(o>20)out.push(`TEXTOVERLAP "${T[a].text.slice(0,25)}" x "${T[b].text.slice(0,25)}"`)}
 console.log('== '+f+' ('+vw+'x'+vh+')');const c={};out.forEach(l=>c[l]=(c[l]||0)+1);Object.entries(c).forEach(([l,n])=>console.log('  '+l+(n>1?' x'+n:'')));
}
