"""Project the original static jacket background into native textured quads."""
import hashlib
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "Assets/Resources/live/view/Background2DView.prefab"
DEFAULT = "Assets/Sekai/assetbundle/resources/startapp/live/2dmode/background/default.asset"


def compile_background(index):
    def asset(path):
        value = index["assets"][path]
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == value["sha256"], path
        return value["objects"]

    objects = asset(SOURCE)
    by_id = {o["id"]: o for o in objects}
    transforms = {o["fields"]["m_GameObject"]["fileID"]: o for o in objects if o["kind"] == "Transform"}
    identity = [[1,0,0,0],[0,1,0,0],[0,0,1,0]]

    def point(matrix, p):
        return [sum(row[i]*p[i] for i in range(3))+row[3] for row in matrix]

    def compose(a, b):
        return [[sum(a[i][k]*b[k][j] for k in range(3)) + (a[i][3] if j == 3 else 0)
                 for j in range(4)] for i in range(3)]

    cache = {}
    def world(go):
        if go in cache:
            return cache[go]
        fields = transforms[go]["fields"]
        x,y,z,w = [fields["m_LocalRotation"][c] for c in "xyzw"]
        rows = [[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],
                [2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],
                [2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]]
        scale = [fields["m_LocalScale"][c] for c in "xyz"]
        local = [[r[j]*scale[j] for j in range(3)] + [fields["m_LocalPosition"][c]]
                 for r,c in zip(rows,"xyz")]
        parent = by_id.get(fields["m_Father"]["fileID"])
        cache[go] = compose(world(parent["fields"]["m_GameObject"]["fileID"]),local) if parent else local
        return cache[go]

    def ancestors(go):
        names=[]
        while go in transforms:
            names.append(by_id[go]["fields"]["m_Name"])
            parent=by_id.get(transforms[go]["fields"]["m_Father"]["fileID"])
            go=parent["fields"]["m_GameObject"]["fileID"] if parent else None
        return names

    camera = next(o["fields"] for o in objects if o["kind"]=="Camera" and "jacketCamera" in ancestors(o["fields"]["m_GameObject"]["fileID"]))
    camera_world = world(camera["m_GameObject"]["fileID"])
    assert [row[:3] for row in camera_world] == [row[:3] for row in identity]
    assert not camera["orthographic"]
    camera_position = [row[3] for row in camera_world]
    focal = 540/math.tan(math.radians(camera["field of view"]*.5))

    def project(go, size, pivot=(.5,.5)):
        matrix = world(go)
        corners = [(-pivot[0]*size[0],(1-pivot[1])*size[1],0),
                   ((1-pivot[0])*size[0],(1-pivot[1])*size[1],0),
                   ((1-pivot[0])*size[0],-pivot[1]*size[1],0),
                   (-pivot[0]*size[0],-pivot[1]*size[1],0)]
        xy=[]
        for corner in corners:
            p=[a-b for a,b in zip(point(matrix,corner),camera_position)]
            assert p[2]>0
            xy.extend([960+p[0]*focal/p[2],540-p[1]*focal/p[2]])
        return xy

    def sprite(guid):
        fields = next(o["fields"] for o in asset(index["guid_paths"][guid]) if o["kind"]=="Sprite")
        rect=fields["m_Rect"]
        return [rect["width"]/fields["m_PixelsToUnits"],rect["height"]/fields["m_PixelsToUnits"]], [fields["m_Pivot"][c] for c in "xy"]

    masks={}
    for obj in objects:
        if obj["kind"]!="SpriteMask":continue
        f=obj["fields"];go=f["m_GameObject"]["fileID"]
        if "jacketContent" not in ancestors(go):continue
        size,pivot=sprite(f["m_Sprite"]["guid"])
        xy=project(go,size,pivot)
        # The masks' projected edges are axis aligned. Keep their texture for
        # later rounded-corner coverage; the native ABI currently clips rectangles.
        x,y=xy[::2],xy[1::2]
        parent=transforms[go]["fields"]["m_Father"]["fileID"]
        masks[parent] = [min(x),min(y),max(x)-min(x),max(y)-min(y)]

    default_guid = next(g for g,p in index["guid_paths"].items() if p==DEFAULT)
    renderers=[]
    for obj in objects:
        if obj["kind"]!="SpriteRenderer":continue
        f=obj["fields"];go=f["m_GameObject"]["fileID"]
        chain=ancestors(go)
        if "jacketContent" not in chain or not f["m_Enabled"]:continue
        name=by_id[go]["fields"]["m_Name"]
        guid=f["m_Sprite"].get("guid")
        jacket=name=="Jacket"
        if name=="Background":guid=default_guid
        if jacket:
            # Normalize local covers to the source jacket's authored 7.4-unit
            # square; image resolution should not change its onscreen size.
            size,pivot=[7.4,7.4],[.5,.5]
        elif guid:
            size,pivot=sprite(guid)
        else:
            continue
        assert f["m_DrawMode"]==0
        xy=project(go,size,pivot)
        parent=transforms[go]["fields"]["m_Father"]["fileID"]
        clip=masks.get(parent,[0,0,1920,1080]) if f["m_MaskInteraction"] else [0,0,1920,1080]
        renderers.append({"name":"/".join(reversed(chain)),"guid":guid or "", "jacket":jacket,
                          "base":name=="Background",
                          "order":f["m_SortingOrder"],"xy":xy,"clip":clip,
                          "color":[f["m_Color"][c] for c in "rgba"]})
    return {"source":SOURCE,"sprites":sorted(renderers,key=lambda r:r["order"]),
            "limitations":["Native rectangular clipping does not yet reproduce rounded SpriteMask coverage."]}
