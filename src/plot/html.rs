//! Interactive HTML output: one self-contained file (no network access, no
//! external scripts) with the figure as SVG and a small script for hover
//! readouts, a crosshair with axis values, hiding series from the legend
//! and saving a PNG; 3D plots turn with the mouse (drag), zoom with the
//! wheel and reset on double click. `fig.save("chart.html")` or
//! [`Figure::to_html`].

use super::axes::{AxesKind, Layout};
use super::figure::Figure;
use super::scale::Scale;
use super::series::short;
use std::fmt::Write;

/// Points per series kept for hover readouts.
const HOVER_POINTS: usize = 2000;

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '<' => o.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn num(v: f64) -> String {
    if v.is_finite() {
        let r = (v * 100.0).round() / 100.0;
        format!("{r}")
    } else {
        "null".into()
    }
}

/// Axis description for the script: pixel ends, data limits, scale.
fn axis_json(map: &super::axis::AxisMap, categories: &[String]) -> String {
    let scale = match map.scale {
        Scale::Log(_) => "log",
        Scale::Time => "time",
        Scale::Category => "cat",
        _ => "lin",
    };
    let cats: Vec<String> = categories.iter().map(|c| json_str(c)).collect();
    format!(
        r#"{{"p0":{},"p1":{},"min":{},"max":{},"scale":"{scale}","cats":[{}]}}"#,
        num(map.p0),
        num(map.p1),
        map.min,
        map.max,
        cats.join(",")
    )
}

impl Figure {
    /// The figure as an interactive, self-contained HTML page (see
    /// [`crate::plot::html`]).
    pub fn to_html(&mut self) -> String {
        let scene = self.accessible_scene();
        let layouts = self.last_layouts.clone();
        let svg = scene.to_svg();
        let mut plots = Vec::new();
        for (i, lay) in layouts.iter().enumerate() {
            if let Some(p) = self.plot_json(i, lay) {
                plots.push(p);
            }
        }
        let title = self.title_text().unwrap_or_else(|| "Chart".into());
        let meta = format!(r#"{{"plots":[{}]}}"#, plots.join(","));
        let three = if plots.iter().any(|p| p.contains(r#""three":"#)) {
            format!("<script>{SCRIPT3D}</script>")
        } else {
            String::new()
        };
        let mut html = String::with_capacity(svg.len() + meta.len() + 8192);
        let _ = write!(
            html,
            r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{}</title>
<style>
body{{margin:0;padding:16px;background:#fff;font-family:system-ui,sans-serif}}
#dv{{position:relative;max-width:{}px}}
#dv svg{{width:100%;height:auto;display:block;cursor:crosshair}}
#dv .bar{{position:absolute;top:6px;right:6px;opacity:.35;transition:opacity .2s}}
#dv:hover .bar{{opacity:1}}
#dv button{{font:12px system-ui,sans-serif;padding:3px 8px;border:1px solid #bbb;border-radius:4px;background:#fff;cursor:pointer}}
</style></head><body>
<div id="dv"><div class="bar"><button id="dv-png" title="Save as PNG">PNG</button></div>
{svg}</div>
<script type="application/json" id="dv-meta">{meta}</script>
<script>{SCRIPT}</script>
{three}
</body></html>
"#,
            super::scene::esc(&title),
            scene.width.round(),
        );
        html
    }

    fn plot_json(&mut self, i: usize, lay: &Layout) -> Option<String> {
        let theme = self.theme.clone();
        let ax = self.axes_at(i)?;
        if ax.kind == AxesKind::ThreeD {
            let clip = lay.clip3d.unwrap_or(lay.plot);
            return Some(format!(
                r#"{{"gid":"p{i}-s3d","three":{}}}"#,
                ax.export3d(lay.plot, clip, &theme)
            ));
        }
        if ax.kind != AxesKind::Cartesian {
            return None;
        }
        let p = lay.plot;
        let mut series = Vec::new();
        for (k, s) in ax.series.iter().enumerate() {
            if !ax.is_visible(k) {
                continue;
            }
            let pts = s.data_points(HOVER_POINTS);
            if pts.is_empty() {
                continue;
            }
            let y = if s.secondary() {
                lay.y2.as_ref().unwrap_or(&lay.y)
            } else {
                &lay.y
            };
            let mut px = String::new();
            let mut labels = String::new();
            let mut first = true;
            for (dx, dy) in pts {
                let (cx, cy) = (lay.x.px(dx), y.px(dy));
                if !cx.is_finite() || !cy.is_finite() {
                    continue;
                }
                if !first {
                    px.push(',');
                    labels.push(',');
                }
                first = false;
                let _ = write!(px, "{},{}", num(cx), num(cy));
                let xl = ax.cursor_label(lay, false, dx);
                labels.push_str(&json_str(&format!("{xl}\u{1f}{}", short(dy))));
            }
            let name = s.name().unwrap_or_else(|| format!("series {}", k + 1));
            series.push(format!(
                r#"{{"id":"p{i}-s{k}","name":{},"xy":[{px}],"labels":[{labels}]}}"#,
                json_str(&name)
            ));
        }
        let legend: Vec<String> = lay
            .legend_items
            .iter()
            .map(|(r, k)| {
                format!(
                    r#"{{"r":[{},{},{},{}],"id":"p{i}-s{k}"}}"#,
                    num(r.x),
                    num(r.y),
                    num(r.w),
                    num(r.h)
                )
            })
            .collect();
        Some(format!(
            r#"{{"rect":[{},{},{},{}],"x":{},"y":{},"series":[{}],"legend":[{}]}}"#,
            num(p.x),
            num(p.y),
            num(p.w),
            num(p.h),
            axis_json(&lay.x, &ax.x.categories),
            axis_json(&lay.y, &ax.y.categories),
            series.join(","),
            legend.join(",")
        ))
    }
}

/// The page script (no dependencies).
const SCRIPT: &str = r#"(()=>{
const M=JSON.parse(document.getElementById('dv-meta').textContent);
M.plots=M.plots.filter(p=>!p.three);
const box=document.getElementById('dv'),svg=box.querySelector('svg'),NS='http://www.w3.org/2000/svg';
const ov=document.createElementNS(NS,'g');ov.setAttribute('pointer-events','none');svg.appendChild(ov);
const dims=document.createElementNS(NS,'g');dims.setAttribute('pointer-events','none');svg.appendChild(dims);
const hidden=new Set();
function el(t,a,p){const e=document.createElementNS(NS,t);for(const k in a)e.setAttribute(k,a[k]);(p||ov).appendChild(e);return e;}
function pos(ev){const r=svg.getBoundingClientRect(),vb=svg.viewBox.baseVal;return[(ev.clientX-r.left)*vb.width/r.width,(ev.clientY-r.top)*vb.height/r.height];}
function value(a,px){const t=(px-a.p0)/(a.p1-a.p0);if(a.scale==='log'){const l0=Math.log10(a.min),l1=Math.log10(a.max);return Math.pow(10,l0+t*(l1-l0));}return a.min+t*(a.max-a.min);}
function two(n){return String(n).padStart(2,'0');}
function fmt(a,v){const span=Math.abs(a.max-a.min);
 if(a.scale==='cat'){const i=Math.round(v);return a.cats[i]!==undefined?a.cats[i]:'';}
 if(a.scale==='time'){const d=new Date(v*1000),D=d.getUTCFullYear()+'-'+two(d.getUTCMonth()+1)+'-'+two(d.getUTCDate()),T=two(d.getUTCHours())+':'+two(d.getUTCMinutes())+':'+two(d.getUTCSeconds());return span<120?T:span<2*86400?D.slice(5)+' '+T:D+' '+T.slice(0,5);}
 if(a.scale==='log')return v.toPrecision(3);
 const dig=Math.min(12,Math.max(0,3-Math.floor(Math.log10(span||1))));return v.toFixed(dig);}
function label(x,y,text,anchor){const t=el('text',{x,y,'font-size':11,fill:'#fff','text-anchor':anchor,'dominant-baseline':'middle'});t.textContent=text;const b=t.getBBox();const r=el('rect',{x:b.x-5,y:b.y-2,width:b.width+10,height:b.height+4,rx:2,fill:'rgba(50,50,55,.92)'});ov.insertBefore(r,t);}
function tip(x,y,lines,P){const g=el('g',{});const pad=8,lh=16;const ts=lines.map((s,i)=>{const t=el('text',{x:0,y:pad+lh*(i+.5),'font-size':12,fill:'#141414','dominant-baseline':'middle','font-weight':i?400:600},g);t.textContent=s;return t;});
 const w=Math.max(...ts.map(t=>t.getBBox().width))+2*pad,h=lines.length*lh+pad;let tx=x+12;if(tx+w>P[0]+P[2])tx=x-12-w;tx=Math.max(P[0],tx);let ty=Math.min(Math.max(P[1],y-h/2),P[1]+P[3]-h);
 const r=el('rect',{x:0,y:0,width:w,height:h,rx:4,fill:'rgba(255,255,255,.96)',stroke:'#888'},g);g.insertBefore(r,g.firstChild);ts.forEach(t=>t.setAttribute('x',pad));g.setAttribute('transform','translate('+tx+','+ty+')');}
function inside(P,m){return m[0]>=P.rect[0]&&m[0]<=P.rect[0]+P.rect[2]&&m[1]>=P.rect[1]&&m[1]<=P.rect[1]+P.rect[3];}
svg.addEventListener('mousemove',ev=>{ov.replaceChildren();const m=pos(ev);const P=M.plots.find(p=>inside(p,m));if(!P)return;const R=P.rect;
 const dash={stroke:'rgba(90,90,90,.8)','stroke-width':1,'stroke-dasharray':'4 3'};
 el('line',Object.assign({x1:m[0],x2:m[0],y1:R[1],y2:R[1]+R[3]},dash));el('line',Object.assign({x1:R[0],x2:R[0]+R[2],y1:m[1],y2:m[1]},dash));
 label(m[0],R[1]+R[3]+10,fmt(P.x,value(P.x,m[0])),'middle');label(R[0]-4,m[1],fmt(P.y,value(P.y,m[1])),'end');
 readout(P,m[0],m[1],true);
 // Linked cursor: plots showing (about) the same x range.
 const v=value(P.x,m[0]);
 for(const Q of M.plots){if(Q===P||Q.x.scale!==P.x.scale||Q.x.scale==='log')continue;
  const ov_=Math.min(Q.x.max,P.x.max)-Math.max(Q.x.min,P.x.min),un=Math.max(Q.x.max,P.x.max)-Math.min(Q.x.min,P.x.min);if(!(un>0&&ov_>=0.9*un))continue;
  const qx=Q.x.p0+(v-Q.x.min)/(Q.x.max-Q.x.min)*(Q.x.p1-Q.x.p0);const S=Q.rect;if(qx<S[0]||qx>S[0]+S[2])continue;
  el('line',Object.assign({x1:qx,x2:qx,y1:S[1],y2:S[1]+S[3]},dash));readout(Q,qx,S[1]+4,false);}
});
function readout(P,x,y,dots){const lines=[];let head=null;
 for(const s of P.series){if(hidden.has(s.id))continue;let best=-1,bd=1e9;for(let i=0;i<s.xy.length;i+=2){const d=Math.abs(s.xy[i]-x);if(d<bd){bd=d;best=i;}}
  if(best<0||bd>30)continue;const [xl,yl]=s.labels[best/2].split('\u001f');if(head===null)head=xl;lines.push(s.name+': '+yl);
  el('circle',{cx:s.xy[best],cy:s.xy[best+1],r:4,fill:'#fff',stroke:'#1e1e1e','stroke-width':1.5});}
 if(lines.length)tip(x,y,dots?[head].concat(lines):lines,P.rect);}
svg.addEventListener('mouseleave',()=>ov.replaceChildren());
svg.addEventListener('click',ev=>{const m=pos(ev);for(const P of M.plots)for(const L of P.legend){const r=L.r;if(m[0]>=r[0]&&m[0]<=r[0]+r[2]&&m[1]>=r[1]&&m[1]<=r[1]+r[3]){
  const g=document.getElementById(L.id);const off=!hidden.has(L.id);if(off)hidden.add(L.id);else hidden.delete(L.id);if(g)g.style.display=off?'none':'';
  const k='dim-'+L.id;const old=document.getElementById(k);if(old)old.remove();if(off)el('rect',{id:k,x:r[0],y:r[1],width:r[2],height:r[3],fill:'rgba(255,255,255,.65)'},dims);return;}}});
document.getElementById('dv-png').addEventListener('click',()=>{ov.replaceChildren();const vb=svg.viewBox.baseVal,s=2;const img=new Image();
 img.onload=()=>{const c=document.createElement('canvas');c.width=vb.width*s;c.height=vb.height*s;const x=c.getContext('2d');x.drawImage(img,0,0,c.width,c.height);const a=document.createElement('a');a.download=(document.title||'chart')+'.png';a.href=c.toDataURL('image/png');a.click();};
 img.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(new XMLSerializer().serializeToString(svg));});
})();"#;

/// 3D renderer of the page: the scene's primitives (normalized box
/// coordinates) drawn on a canvas with the camera, lighting, perspective and
/// depth order of dataviz; drag turns, the wheel zooms, double click resets.
const SCRIPT3D: &str = r#"(()=>{
const M=JSON.parse(document.getElementById('dv-meta').textContent);
const svg=document.querySelector('#dv svg'),NS='http://www.w3.org/2000/svg';
const L=[-0.42,0.56,0.71],Hn=un([-0.42,0.56,1.71]);
function un(a){const l=Math.hypot(a[0],a[1],a[2])||1;return[a[0]/l,a[1]/l,a[2]/l];}
function dot(a,b){return a[0]*b[0]+a[1]*b[1]+a[2]*b[2];}
function cross(a,b){return[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];}
function sub(a,b){return[a[0]-b[0],a[1]-b[1],a[2]-b[2]];}
function mul(a,b){return a.map(r=>[0,1,2].map(j=>r[0]*b[0][j]+r[1]*b[1][j]+r[2]*b[2][j]));}
function rot(ax,deg){const t=deg*Math.PI/180,s=Math.sin(t),c=Math.cos(t);return ax===0?[[1,0,0],[0,c,-s],[0,s,c]]:ax===1?[[c,0,s],[0,1,0],[-s,0,c]]:[[c,-s,0],[s,c,0],[0,0,1]];}
function ortho(m){const r0=un(m[0]);const d=dot(m[1],r0);const r1=un(sub(m[1],[r0[0]*d,r0[1]*d,r0[2]*d]));return[r0,r1,cross(r0,r1)];}
function mix(c,d,t){t=Math.max(0,Math.min(1,t));return[c[0]+(d[0]-c[0])*t,c[1]+(d[1]-c[1])*t,c[2]+(d[2]-c[2])*t,c[3]];}
function dark(c,a){return mix(c,[0,0,0,c[3]],a);}
function light(c,a){return mix(c,[255,255,255,c[3]],a);}
function css(c,a){return'rgba('+Math.round(c[0])+','+Math.round(c[1])+','+Math.round(c[2])+','+(a===undefined?c[3]:a)+')';}
function lambert(n){if(n[2]<0)n=[-n[0],-n[1],-n[2]];const d=Math.max(0,dot(n,L));const s=Math.pow(Math.max(0,dot(n,Hn)),28);return[0.42+0.62*d,0.32*s];}
function lit(c,ls){let b=ls[0]<1?dark(c,1-ls[0]):light(c,(ls[0]-1)*0.6);return ls[1]>0.004?light(b,ls[1]):b;}
for(const P of M.plots){if(P.three)setup(P);}
function setup(P){
 const T=P.three,g=document.getElementById(P.gid);if(g)g.style.display='none';
 const [cx,cy,cw,ch]=T.clip;
 const fo=document.createElementNS(NS,'foreignObject');for(const [k,v] of [['x',cx],['y',cy],['width',cw],['height',ch]])fo.setAttribute(k,v);
 const cv=document.createElement('canvas');cv.style.cssText='width:100%;height:100%;display:block;cursor:grab;touch-action:none';fo.appendChild(cv);svg.appendChild(fo);
 const ctx=cv.getContext('2d');const m0=T.m.map(r=>r.slice());let m=m0.map(r=>r.slice()),zoom=1;
 const ext=T.ext,fog=T.fog,bg=T.bg;
 function view(p){return[dot(m[0],p),dot(m[1],p),dot(m[2],p)];}
 function persp(d){return T.dist>0?T.dist/Math.max(T.dist-d,T.radius*0.05):1;}
 function scr(v){const f=persp(v[2]);return[T.center[0]+v[0]*T.scale*zoom*f,T.center[1]-v[1]*T.scale*zoom*f,v[2],f];}
 function cue(c,d){if(fog<=0)return c;const near=Math.max(0,Math.min(1,(d/T.radius+1)/2));return mix(c,[bg[0],bg[1],bg[2],c[3]],fog*(1-near));}
 function resize(){const k=svg.getBoundingClientRect().width/svg.viewBox.baseVal.width,dpr=window.devicePixelRatio||1;cv.width=Math.max(1,Math.round(cw*k*dpr));cv.height=Math.max(1,Math.round(ch*k*dpr));ctx.setTransform(k*dpr,0,0,k*dpr,-cx*k*dpr,-cy*k*dpr);draw();}
 function text(x,y,t,size,c,align,base,rotate,bold){ctx.save();ctx.font=(bold?'bold ':'')+size+'px "DejaVu Sans",system-ui,sans-serif';ctx.fillStyle=css(c);ctx.textAlign=align;ctx.textBaseline=base;ctx.translate(x,y);if(rotate)ctx.rotate(rotate*Math.PI/180);ctx.fillText(t,0,0);ctx.restore();}
 function boxDraw(){if(!T.axes.length)return;const e=ext;
  const far=[0,1,2].map(i=>{const a=[0,0,0];a[i]=1;return view(a)[2]>0?-e[i]:e[i];});
  const at=c=>scr(view(c));
  ctx.lineWidth=1;
  for(let i=0;i<3;i++){const u=(i+1)%3,w=(i+2)%3;const corner=(a,b)=>{const c=[0,0,0];c[i]=far[i];c[u]=a;c[w]=b;return at(c);};
   const q=[corner(-e[u],-e[w]),corner(e[u],-e[w]),corner(e[u],e[w]),corner(-e[u],e[w])];
   ctx.beginPath();q.forEach((p,k)=>k?ctx.lineTo(p[0],p[1]):ctx.moveTo(p[0],p[1]));ctx.closePath();ctx.fillStyle=css(dark(bg,0.035));ctx.fill();ctx.strokeStyle=css(T.axis,0.35*T.axis[3]);ctx.stroke();
   ctx.strokeStyle=css(T.grid);ctx.beginPath();
   for(const [ax,ot] of [[u,w],[w,u]]){for(const t of T.axes[ax].t){const n=t[0];const a=ax===u?corner(n,-e[ot]):corner(-e[ot],n),b=ax===u?corner(n,e[ot]):corner(e[ot],n);ctx.moveTo(a[0],a[1]);ctx.lineTo(b[0],b[1]);}}
   ctx.stroke();}
  const ctr=at([0,0,0]),ts=T.ts*0.85;
  for(let i=0;i<3;i++){const u=(i+1)%3,w=(i+2)%3;const a=[0,0,0];a[i]=1;const dv=view(a);if(Math.hypot(dv[0],dv[1])<0.12)continue;const across=Math.abs(dv[0])>=Math.abs(dv[1]);
   let best=null,bs=-1e9;for(const [su,sw] of [[-1,-1],[1,-1],[1,1],[-1,1]]){const c=[0,0,0];c[u]=su*e[u];c[w]=sw*e[w];const back=(c[u]===far[u])+(c[w]===far[w]);if(back===2)continue;const s=at(c);const sc=across?s[1]:-s[0];if(sc>bs){bs=sc;best=c;}}
   if(!best)continue;const pt=t=>{const c=best.slice();c[i]=t;return at(c);};const m0_=pt(-e[i]),m1=pt(e[i]),mid=[(m0_[0]+m1[0])/2,(m0_[1]+m1[1])/2];
   let ox=mid[0]-ctr[0],oy=mid[1]-ctr[1];const ol=Math.hypot(ox,oy)||1;ox/=ol;oy/=ol;
   ctx.strokeStyle=css(T.axis);ctx.beginPath();ctx.moveTo(m0_[0],m0_[1]);ctx.lineTo(m1[0],m1[1]);ctx.stroke();
   const align=ox<-0.45?'right':ox>0.45?'left':'center';let widest=0;ctx.font=ts+'px "DejaVu Sans",system-ui,sans-serif';
   for(const t of T.axes[i].t){const s=pt(t[0]);ctx.beginPath();ctx.moveTo(s[0],s[1]);ctx.lineTo(s[0]+ox*4,s[1]+oy*4);ctx.stroke();widest=Math.max(widest,ctx.measureText(t[1]).width);text(s[0]+ox*9,s[1]+oy*9,t[1],ts,T.text,align,'middle');}
   const name=T.axes[i].name;if(name){if(across){const d=14+ts*0.6+widest*Math.abs(ox);text(mid[0]+ox*d,mid[1]+oy*d,name,T.ls,T.text,align,'middle');}else{const d=14+widest*Math.abs(ox)+T.ls*0.6;text(mid[0]+ox*d,mid[1]+oy*d,name,T.ls,T.text,'center','middle',-90);}}}
 }
 function draw(){ctx.save();ctx.setTransform(1,0,0,1,0,0);ctx.clearRect(0,0,cv.width,cv.height);ctx.restore();
  boxDraw();const prims=[],labels=[];
  for(const it of T.items){const k=it.k,c0=it.c;
   if(k==='f'){const n=it.p.length/3,v=[];for(let j=0;j<n;j++)v.push(view([it.p[3*j],it.p[3*j+1],it.p[3*j+2]]));
    let nv=it.n?view(it.n):un(cross(sub(v[1],v[0]),sub(v[2],v[0])));if(it.cull&&nv[2]<-0.02)continue;const d=v.reduce((a,q)=>a+q[2],0)/n;
    const col=cue(T.light?lit(c0,lambert(nv)):c0,d);prims.push([d,()=>{ctx.beginPath();v.forEach((q,j)=>{const s=scr(q);j?ctx.lineTo(s[0],s[1]):ctx.moveTo(s[0],s[1]);});ctx.closePath();ctx.fillStyle=css(col);ctx.fill();
     if(it.e){ctx.strokeStyle=css(cue(it.e,d));ctx.lineWidth=it.ew;ctx.stroke();}else if(col[3]>0.99){ctx.strokeStyle=css(col);ctx.lineWidth=0.6;ctx.stroke();}}]);}
   else if(k==='b'){const v=view(it.p),s=scr(v);const r=it.px!==undefined?it.px*s[3]:it.r*T.scale*zoom*s[3];if(!(r>0))continue;const col=cue(c0,v[2]);const sh=it.s&&T.light;
    prims.push([v[2],()=>{if(sh&&r>=1.6){const gx=s[0]+L[0]*0.62*r*0.55,gy=s[1]-L[1]*0.62*r*0.55;const gr=ctx.createRadialGradient(gx,gy,r*0.08,s[0],s[1],r);gr.addColorStop(0,css(light(col,0.6)));gr.addColorStop(0.35,css(col));gr.addColorStop(1,css(dark(col,0.5)));ctx.fillStyle=gr;}else ctx.fillStyle=css(col);ctx.beginPath();ctx.arc(s[0],s[1],r,0,2*Math.PI);ctx.fill();}]);}
   else if(k==='l'||k==='t'){const n=it.p.length/3,pts=[];for(let j=0;j<n;j++)pts.push(view([it.p[3*j],it.p[3*j+1],it.p[3*j+2]]));
    const piece=k==='t'?12:32;for(let a=0;a+1<n;a+=piece){const b=Math.min(a+piece,n-1),seg=pts.slice(a,b+1),d=seg.reduce((x,q)=>x+q[2],0)/seg.length,f=persp(d);
     const col=cue(it.cs?it.cs[Math.floor((a+b)/2)]:c0,d);const w=k==='t'?2*it.r*T.scale*zoom*f:it.w*f;
     prims.push([d,()=>{const sp=seg.map(scr);const path=()=>{ctx.beginPath();sp.forEach((s,j)=>j?ctx.lineTo(s[0],s[1]):ctx.moveTo(s[0],s[1]));};ctx.lineCap='round';ctx.lineJoin='round';
      if(k==='t'&&T.light&&w>3){path();ctx.strokeStyle=css(dark(col,0.3));ctx.lineWidth=w;ctx.stroke();path();ctx.strokeStyle=css(light(col,0.18));ctx.lineWidth=w*0.5;ctx.stroke();}else{path();ctx.strokeStyle=css(col);ctx.lineWidth=Math.max(w,0.3);ctx.stroke();}}]);}}
   else if(k==='a'){const a=view(it.p.slice(0,3)),b=view(it.p.slice(3,6)),d=(a[2]+b[2])/2,col=cue(c0,d);
    prims.push([d,()=>{const sa=scr(a),sb=scr(b);const dx=sb[0]-sa[0],dy=sb[1]-sa[1],l=Math.hypot(dx,dy);if(l<0.5)return;const ux=dx/l,uy=dy/l,h=Math.min(it.h,l*0.6),bx=sb[0]-ux*h,by=sb[1]-uy*h,w=h*0.45;
     ctx.strokeStyle=css(col);ctx.lineWidth=it.w;ctx.lineCap='round';ctx.beginPath();ctx.moveTo(sa[0],sa[1]);ctx.lineTo(bx,by);ctx.stroke();ctx.fillStyle=css(col);ctx.beginPath();ctx.moveTo(sb[0],sb[1]);ctx.lineTo(bx-uy*w,by+ux*w);ctx.lineTo(bx+uy*w,by-ux*w);ctx.closePath();ctx.fill();}]);}
   else if(k==='x')labels.push(it);}
  prims.sort((a,b)=>a[0]-b[0]);for(const p of prims)p[1]();
  for(const it of labels){const s=scr(view(it.p));if(it.bg){ctx.font=(it.bold?'bold ':'')+it.z+'px "DejaVu Sans",system-ui,sans-serif';const w=ctx.measureText(it.t).width+it.z*0.8,h=it.z*1.45;ctx.fillStyle=css(it.bg);ctx.beginPath();if(ctx.roundRect)ctx.roundRect(s[0]-w/2,s[1]-h/2,w,h,h*0.3);else ctx.rect(s[0]-w/2,s[1]-h/2,w,h);ctx.fill();}
   text(s[0],s[1],it.t,it.z,it.c,'center','middle',0,it.bold);}
 }
 let drag=null;
 cv.addEventListener('pointerdown',e=>{drag=[e.clientX,e.clientY];cv.setPointerCapture(e.pointerId);cv.style.cursor='grabbing';});
 cv.addEventListener('pointermove',e=>{if(!drag)return;const k=svg.viewBox.baseVal.width/svg.getBoundingClientRect().width;const dx=(e.clientX-drag[0])*k,dy=(e.clientY-drag[1])*k;drag=[e.clientX,e.clientY];
  m=ortho(mul(m,rot(2,dx*0.5)));m=ortho(mul(rot(0,dy*0.5),m));draw();});
 const up=()=>{drag=null;cv.style.cursor='grab';};cv.addEventListener('pointerup',up);cv.addEventListener('pointercancel',up);
 cv.addEventListener('wheel',e=>{e.preventDefault();zoom=Math.max(0.05,Math.min(50,zoom*(e.deltaY<0?1/0.85:0.85)));draw();},{passive:false});
 cv.addEventListener('dblclick',()=>{m=m0.map(r=>r.slice());zoom=1;draw();});
 if(window.ResizeObserver)new ResizeObserver(resize).observe(svg);else window.addEventListener('resize',resize);
 resize();
}
})();"#;

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn html_is_self_contained() {
        let mut fig = Figure::new(600, 400);
        fig.title("Sales <2025>");
        fig.axes()
            .line(vec![0.0, 1.0, 2.0], vec![1.0, 3.0, 2.0])
            .label("a \"quoted\" line");
        fig.axes()
            .bar_xy(vec![0.0, 1.0, 2.0], vec![0.5, 1.0, 1.5])
            .label("bars");
        let html = fig.to_html();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<title>Sales &lt;2025&gt;</title>"));
        assert!(html.contains(r#"id="p0-s0""#) && html.contains(r#"id="p0-s1""#));
        assert!(html.contains(r#""name":"a \"quoted\" line""#));
        assert!(!html.contains("<script src"));
        // The metadata is valid enough to find the series points.
        assert!(html.contains(r#""xy":["#));
    }

    #[test]
    fn three_d_plots_are_exported_for_the_browser() {
        let mut fig = Figure::new(600, 400);
        fig.grid(1, 2);
        fig.at(0, 0)
            .surface_fn(|x, y| x * y, (-1.0, 1.0), (-1.0, 1.0), 6);
        fig.at(0, 0).x_label("x");
        fig.at(0, 1).dna_helix("ATGC");
        let html = fig.to_html();
        assert!(html.contains(r#""gid":"p0-s3d""#) && html.contains(r#""gid":"p1-s3d""#));
        assert!(
            html.contains(r#""k":"f""#)
                && html.contains(r#""k":"t""#)
                && html.contains(r#""k":"b""#)
        );
        assert!(html.contains("createRadialGradient"));
        let meta = html
            .split(r#"id="dv-meta">"#)
            .nth(1)
            .unwrap()
            .split("</script>")
            .next()
            .unwrap();
        // Balanced JSON (no stray quotes from labels).
        assert_eq!(meta.matches('{').count(), meta.matches('}').count());
    }
}
