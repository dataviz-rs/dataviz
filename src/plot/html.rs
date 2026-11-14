//! Interactive HTML output: one self-contained file (no network access, no
//! external scripts) with the figure as SVG and a small script for hover
//! readouts, a crosshair with axis values, hiding series from the legend
//! and saving a PNG. `fig.save("chart.html")` or [`Figure::to_html`].

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
        let (scene, layouts) = self.scene_and_layouts();
        let svg = scene.to_svg();
        let mut plots = Vec::new();
        for (i, lay) in layouts.iter().enumerate() {
            if let Some(p) = self.plot_json(i, lay) {
                plots.push(p);
            }
        }
        let title = self.title_text().unwrap_or_else(|| "Chart".into());
        let meta = format!(r#"{{"plots":[{}]}}"#, plots.join(","));
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
</body></html>
"#,
            super::scene::esc(&title),
            scene.width.round(),
        );
        html
    }

    fn plot_json(&mut self, i: usize, lay: &Layout) -> Option<String> {
        let ax = self.axes_at(i)?;
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
 const lines=[];let head=null;
 for(const s of P.series){if(hidden.has(s.id))continue;let best=-1,bd=1e9;for(let i=0;i<s.xy.length;i+=2){const d=Math.abs(s.xy[i]-m[0]);if(d<bd){bd=d;best=i;}}
  if(best<0||bd>30)continue;const [xl,yl]=s.labels[best/2].split('\u001f');if(head===null)head=xl;lines.push(s.name+': '+yl);
  el('circle',{cx:s.xy[best],cy:s.xy[best+1],r:4,fill:'#fff',stroke:'#1e1e1e','stroke-width':1.5});}
 if(lines.length)tip(m[0],m[1],[head].concat(lines),R);});
svg.addEventListener('mouseleave',()=>ov.replaceChildren());
svg.addEventListener('click',ev=>{const m=pos(ev);for(const P of M.plots)for(const L of P.legend){const r=L.r;if(m[0]>=r[0]&&m[0]<=r[0]+r[2]&&m[1]>=r[1]&&m[1]<=r[1]+r[3]){
  const g=document.getElementById(L.id);const off=!hidden.has(L.id);if(off)hidden.add(L.id);else hidden.delete(L.id);if(g)g.style.display=off?'none':'';
  const k='dim-'+L.id;const old=document.getElementById(k);if(old)old.remove();if(off)el('rect',{id:k,x:r[0],y:r[1],width:r[2],height:r[3],fill:'rgba(255,255,255,.65)'},dims);return;}}});
document.getElementById('dv-png').addEventListener('click',()=>{ov.replaceChildren();const vb=svg.viewBox.baseVal,s=2;const img=new Image();
 img.onload=()=>{const c=document.createElement('canvas');c.width=vb.width*s;c.height=vb.height*s;const x=c.getContext('2d');x.drawImage(img,0,0,c.width,c.height);const a=document.createElement('a');a.download=(document.title||'chart')+'.png';a.href=c.toDataURL('image/png');a.click();};
 img.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(new XMLSerializer().serializeToString(svg));});
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
}
