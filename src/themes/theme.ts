import type {Appearance} from '../types';
export const defaultAppearance:Appearance={accent:'#43d3e8',background:'#080b12',density:'comfortable',radius:12,reduce_motion:false,gradient:true,sidebar:'icons',bookmark_bar:'toolbar'};
export const presets=[{name:'ELIX Midnight',accent:'#43d3e8',background:'#080b12'},{name:'Obsidian',accent:'#a78bfa',background:'#121016'},{name:'Frost',accent:'#1768cf',background:'#f4f7fb'},{name:'Forest',accent:'#47de9c',background:'#081813'},{name:'Ember',accent:'#ff9c58',background:'#1a1010'}];
const rgb=(hex:string)=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));
const hex=(values:number[])=>'#'+values.map(v=>Math.round(v).toString(16).padStart(2,'0')).join('');
export const mix=(a:string,b:string,amount:number)=>hex(rgb(a).map((n,i)=>n*(1-amount)+rgb(b)[i]*amount));
export function luminance(color:string){const [r,g,b]=rgb(color).map(n=>{const c=n/255;return c<=.04045?c/12.92:((c+.055)/1.055)**2.4});return .2126*r+.7152*g+.0722*b}
export function contrast(a:string,b:string){const x=luminance(a),y=luminance(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05)}
export const foreground=(bg:string)=>contrast(bg,'#ffffff')>contrast(bg,'#000000')?'#ffffff':'#000000';
export function themeVariables(a:Appearance){
 const bg=a.background,text=foreground(bg),light=text==='#000000';let accent=a.accent;
 for(let step=0;step<60&&contrast(accent,bg)<4.5;step++)accent=mix(accent,text,.12);
 return {'--bg':bg,'--surface':mix(bg,text,.035),'--raised':mix(bg,text,.075),'--hover':mix(bg,text,.13),'--text':text,'--muted':mix(bg,text,.72),'--subtle-text':mix(bg,text,.60),'--border':mix(bg,text,.22),'--accent':accent,'--accent-raw':a.accent,'--on-accent':foreground(accent),'--accent-soft':mix(bg,accent,.13),'--radius':`${a.radius}px`,'--scheme':light?'light':'dark'};
}
export function applyTheme(appearance:Appearance){if(typeof document==='undefined')return;const root=document.documentElement;for(const [key,value]of Object.entries(themeVariables(appearance)))root.style.setProperty(key,value);root.dataset.density=appearance.density;root.dataset.reduceMotion=String(appearance.reduce_motion);root.dataset.gradient=String(appearance.gradient)}
