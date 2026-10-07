#!/usr/bin/env python3
"""Author atlas coordinates, joints and clips; never alter generated PNG pixels."""
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "resources/generated/results/rig-v1"
SOURCE = ASSETS / "atlas-v2.png"


def main():
    image = Image.open(SOURCE).convert("RGBA")
    regions = [
        ("torso",(0,0,305,425)), ("head",(306,0,670,404)),
        ("eyes_open",(671,170,938,350)), ("eyes_closed",(949,171,1254,350)),
        ("upper_left",(60,425,267,760)), ("fore_left",(350,425,610,760)),
        ("upper_right",(677,425,905,760)), ("fore_right",(955,425,1254,760)),
        ("hair_left",(0,755,285,1019)), ("hair_right",(334,755,648,1019)),
        ("tie_left",(675,751,938,1023)), ("tie_right",(950,751,1254,1023)),
        ("bow_left",(0,1024,341,1254)), ("bow_right",(348,1024,676,1254)),
        ("mouth_closed",(684,1045,936,1254)), ("mouth_open",(965,1045,1254,1254)),
    ]
    assert image.size == (1254,1254), "Reauthor regions when the generated atlas changes"
    boxes = {}
    for name,(l,t,r,b) in regions:
        # Low-alpha background residues are excluded only from measurement.
        # The generated RGBA image and its antialiasing remain untouched.
        points = [(x,y) for y in range(t,b) for x in range(l,r) if image.getpixel((x,y))[3] >= 64]
        assert points, name
        x=max(l,min(x for x,y in points)-2);y=max(t,min(y for x,y in points)-2)
        right=min(r,max(x for x,y in points)+3);bottom=min(b,max(y for x,y in points)+3)
        boxes[name] = [x,y,right-x,bottom-y]
    # Expression variants keep brows at the same vertical coordinate.
    boxes["eyes_closed"][1] = boxes["eyes_open"][1]
    boxes["eyes_closed"][3] = boxes["eyes_open"][3]
    bones = []
    def bone(name,parent,translation):
        bones.append({"name":name,"parent":parent,"translation":translation})
        return len(bones)-1
    body=bone("body",None,[512,1070]);head=bone("head",body,[0,-385])
    layers=[]
    def layer(name,joint,rect,source=None,variant=None):
        value={"name":name,"bone":joint,"source":boxes[source or name],"rect":rect}
        if variant:value["variant"]=variant
        layers.append(value)
    def anchor(name,at_bottom):
        x,y,w,h=boxes[name];row=y+h-15 if at_bottom else y+18
        points=[px for px in range(x,x+w) if image.getpixel((px,row))[3]>=96]
        assert points,name
        return [((min(points)+max(points))*.5-x)/w,(row-y)/h]
    # The elbow pivot is measured on both participating images, then shared
    # by parent and child. Rotation cannot pull those joint centers apart.
    for side,sign in [("left",-1),("right",1)]:
        upper=bone(f"upper_{side}",body,[sign*195,-350])
        width,height=175,387
        shoulder=anchor(f"upper_{side}",False);elbow=anchor(f"upper_{side}",True)
        rect=[-shoulder[0]*width,-shoulder[1]*height,width,height]
        layer(f"upper_{side}",upper,rect)
        fore=bone(f"fore_{side}",upper,[(elbow[0]-shoulder[0])*width,(elbow[1]-shoulder[1])*height])
        p=anchor(f"fore_{side}",True)
        layer(f"fore_{side}",fore,[-p[0]*191,-p[1]*330,191,330])
    fore_layers=layers[1::2];layers=layers[::2]
    layer("torso",body,[-285,-410,570,814])
    layer("head",head,[-280,-475,575,529])
    layers+=fore_layers
    left=bone("hair_left",head,[-190,-180]);right=bone("hair_right",head,[200,-180])
    layer("hair_left",left,[-110,-15,210,236]);layer("hair_right",right,[-95,-15,210,232])
    left=bone("tie_left",body,[0,-114]);right=bone("tie_right",body,[0,-114])
    layer("tie_left",left,[-218,-20,248,320],"tie_right")
    layer("tie_right",right,[-30,-20,248,320],"tie_left")
    left=bone("bow_left",head,[-3,-485]);right=bone("bow_right",head,[4,-485])
    layer("bow_left",left,[-302,-202,320,215]);layer("bow_right",right,[-15,-204,326,215])
    eyes=bone("eyes",head,[0,-175]);mouth=bone("mouth",head,[0,-75])
    layer("eyes_open",eyes,[-137,-60,274,125],variant=["eyes","open"])
    layer("eyes_closed",eyes,[-137,-60,274,125],variant=["eyes","closed"])
    layer("mouth_closed",mouth,[-52,-16,104,32],variant=["mouth","closed"])
    layer("mouth_open",mouth,[-52,-30,104,60],variant=["mouth","open"])
    ids={b["name"]:i for i,b in enumerate(bones)}
    def curve(values):
        return [{"time":t,"value":v,"incoming":0,"outgoing":0,"in_weight":1/3,
                 "out_weight":1/3,"step":False,"step_in":False} for t,v in values]
    def track(name,prop,values):return {"bone":ids[name],"property":prop,"keys":curve(values)}
    def cycle(name,prop,values):
        return track(name,prop,[(i*6.4/(len(values)-1),v) for i,v in enumerate(values)])
    idle_tracks=[cycle("body","scale_y",[1,1.008,1,.996,1]),
                 cycle("head","rotation",[-1,1.4,0,-1.8,-1]),
                 cycle("head","scale_x",[1,.988,1,.992,1]),
                 cycle("fore_left","rotation",[0,1.8,0,-1.1,0]),
                 cycle("fore_right","rotation",[0,-1.3,0,1.7,0])]
    for name,sign in [("hair_left",-1),("hair_right",1),("bow_left",-1),("bow_right",1),("tie_left",-1),("tie_right",1)]:
        idle_tracks.append(cycle(name,"rotation",[0,sign*2,0,-sign*1.2,0]))
    celebrate_tracks=[cycle("body","scale_y",[1,1.008,1,.996,1]),
                      cycle("head","rotation",[-1.5,3.4,0,-3.5,-1.5]),
                      cycle("head","scale_x",[1,.96,1,.975,1]),
                      cycle("upper_left","rotation",[0,-2.5,1.5,-1,0]),
                      cycle("upper_right","rotation",[0,2.5,-1.5,1,0]),
                      cycle("fore_left","rotation",[0,-7,3,-4,0]),
                      cycle("fore_right","rotation",[0,7,-3,4,0])]
    for name,sign in [("hair_left",-1),("hair_right",1),("bow_left",-1),("bow_right",1),("tie_left",-1),("tie_right",1)]:
        celebrate_tracks.append(cycle(name,"rotation",[0,sign*4.5,-sign*1.5,-sign*3,0]))
    def slot(values):return [{"time":t,"value":v} for t,v in values]
    clips={
        "idle":{"duration":6.4,"repeat":True,"tracks":idle_tracks,"slots":{
            "eyes":slot([(0,"open"),(2.9,"closed"),(3.05,"open"),(5.6,"closed"),(5.75,"open")]),
            "mouth":slot([(0,"closed")])}},
        "celebrate":{"duration":6.4,"repeat":True,"tracks":celebrate_tracks,"slots":{
            "eyes":slot([(0,"closed"),(.85,"open"),(1.15,"closed"),(1.3,"open"),(2.8,"closed"),(3.1,"open"),(4.05,"closed"),(5.1,"open"),(5.35,"closed"),(5.5,"open"),(6,"closed")]),
            "mouth":slot([(0,"closed"),(1.8,"open"),(2.3,"closed"),(4.4,"open"),(4.8,"closed")])}}
    }
    definition={"schema":1,"atlas":SOURCE.name,"atlas_size":list(image.size),"canvas":[1024,1536],
                "provenance":{"tool":"built-in image_gen.imagegen","sha256":hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
                              "reference":"resources/references/0114_01/frames/result.png",
                              "source":"Generated reconstruction; not the original character model"},
                "bones":bones,"layers":layers,"clips":clips}
    (ASSETS/"rig.json").write_text(json.dumps(definition,ensure_ascii=False,indent=2)+"\n")
    print(f"2D result rig: {len(bones)} joints, {len(layers)} layers, {len(clips)} clips; {ASSETS/'rig.json'}")


if __name__=="__main__":main()
