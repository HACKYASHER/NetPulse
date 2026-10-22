import QtQuick
Item {
 id: flag
 property string name: ""
 property string code: ""
 implicitWidth: 27; implicitHeight: 18
 function country(n) {
  var points=Array.from(n), result=""
  for (var i=0;i<points.length;i++) { var cp=points[i].codePointAt(0); if(cp>=0x1f1e6&&cp<=0x1f1ff) {result+=String.fromCharCode(cp-0x1f1e6+65);if(result.length===2)return result} else result="" }
  var iso=n.match(/^\s*(FI|DE|RU|US|NL|FR|GB|SE|PL|TR|JP|SG|KZ|EE|LV|LT|UA|CA|CH|NO|DK|AT|CZ|IT|ES|CN|AU|KR|IN)\b/i)
  if(iso)return iso[1].toUpperCase()
  var list=[[/finland|финлянд/i,"FI"],[/germany|германи/i,"DE"],[/netherlands|нидерланд/i,"NL"],[/france|франци/i,"FR"],[/sweden|швеци/i,"SE"],[/russia|росси/i,"RU"],[/united states|сша|america/i,"US"],[/united kingdom|britain|лондон|великобрит/i,"GB"],[/japan|япони/i,"JP"],[/turkey|турци/i,"TR"],[/singapore|сингапур/i,"SG"],[/poland|польш/i,"PL"]]
  for(var j=0;j<list.length;j++)if(list[j][0].test(n))return list[j][1]
  return ""
 }
 property var available: "ad,ae,af,ag,ai,al,am,ao,aq,ar,as,at,au,aw,ax,az,ba,bb,bd,be,bf,bg,bh,bi,bj,bl,bm,bn,bo,bq,br,bs,bt,bv,bw,by,bz,ca,cc,cd,cf,cg,ch,ci,ck,cl,cm,cn,co,cp,cr,cu,cv,cw,cx,cy,cz,de,dg,dj,dk,dm,do,dz,ec,ee,eg,eh,er,es,et,eu,fi,fj,fk,fm,fo,fr,ga,gb,gd,ge,gf,gg,gh,gi,gl,gm,gn,gp,gq,gr,gs,gt,gu,gw,gy,hk,hm,hn,hr,ht,hu,ic,id,ie,il,im,in,io,iq,ir,is,it,je,jm,jo,jp,ke,kg,kh,ki,km,kn,kp,kr,kw,ky,kz,la,lb,lc,li,lk,lr,ls,lt,lu,lv,ly,ma,mc,md,me,mf,mg,mh,mk,ml,mm,mn,mo,mp,mq,mr,ms,mt,mu,mv,mw,mx,my,mz,na,nc,ne,nf,ng,ni,nl,no,np,nr,nu,nz,om,pa,pc,pe,pf,pg,ph,pk,pl,pm,pn,pr,ps,pt,pw,py,qa,re,ro,rs,ru,rw,sa,sb,sc,sd,se,sg,sh,si,sj,sk,sl,sm,sn,so,sr,ss,st,sv,sx,sy,sz,tc,td,tf,tg,th,tj,tk,tl,tm,tn,to,tr,tt,tv,tw,tz,ua,ug,um,un,us,uy,uz,va,vc,ve,vg,vi,vn,vu,wf,ws,xk,xx,ye,yt,za,zm,zw".split(",")
 function svg() {
  var k=(code||country(name)).toLowerCase()
  if(available.indexOf(k)>=0)return Qt.resolvedUrl("../resources/flags/"+k+".svg")
  return 'data:image/svg+xml;utf8,'+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect width="30" height="20" rx="3" fill="#536079"/><circle cx="15" cy="10" r="7" fill="none" stroke="white"/><path d="M8 10h14M15 3v14" stroke="white"/></svg>')
 }

 Image { anchors.fill: parent; source: flag.svg(); fillMode: Image.PreserveAspectFit; asynchronous: false }
}
