"""Builds the 3D objects for noctiluca and exports them as GLB files.

Every model is made from code, so it can be rebuilt or changed at any time.
The style is stylised, not anatomical: simple bold forms that read from the
back of a dark room and look good in chrome or glass.

Run with Blender 4.2 or newer, in the background:

    Blender -b --factory-startup -P blender/build_models.py -- \
        assets/models  previews  [head skull prism ...]

The first folder receives NAME.glb, the second NAME_chrome.png and
NAME_glass.png preview renders (leave it out with "-" to skip rendering).
Any names after that build only those models.

Parts whose object name ends in "_glow" are exported too; noctiluca lights
them up with the palette colour instead of giving them chrome or glass.
"""

import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Vector

# ----------------------------------------------------------------- helpers


def scene():
    return bpy.context.scene


def clear():
    """Removes every object and the data left behind."""
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.curves, bpy.data.materials, bpy.data.cameras, bpy.data.lights):
        for block in list(coll):
            if block.users == 0:
                coll.remove(block)


def select_only(objs):
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objs:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]


def bake(obj):
    """Applies the object's transform and all its modifiers to the mesh."""
    select_only([obj])
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for mod in list(obj.modifiers):
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return obj


def modifier(obj, kind, **settings):
    mod = obj.modifiers.new(kind.lower(), kind)
    for key, value in settings.items():
        setattr(mod, key, value)
    return mod


def join(objs, name):
    for obj in objs:
        bake(obj)
    select_only(objs)
    bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    obj.name = name
    return obj


def remesh(obj, voxel):
    """Rebuilds the surface from voxels: fuses overlapping parts into one skin."""
    modifier(obj, "REMESH", mode="VOXEL", voxel_size=voxel)
    return bake(obj)


def carve(obj, cutter):
    modifier(obj, "BOOLEAN", operation="DIFFERENCE", solver="EXACT", object=cutter)
    bake(obj)
    bpy.data.objects.remove(cutter, do_unlink=True)
    return obj


def smooth(obj, factor=0.5, repeat=4):
    modifier(obj, "SMOOTH", factor=factor, iterations=repeat)
    return bake(obj)


def decimate(obj, faces):
    """Reduces to about `faces` faces. Low counts give the faceted look."""
    count = len(obj.data.polygons)
    if count > faces:
        modifier(obj, "DECIMATE", decimate_type="COLLAPSE", ratio=faces / count)
        bake(obj)
    return obj


def shade(obj, flat):
    values = [not flat] * len(obj.data.polygons)
    obj.data.polygons.foreach_set("use_smooth", values)
    obj.data.update()
    return obj


def sphere(radius, loc=(0, 0, 0), scale=(1, 1, 1), segments=48, rings=24):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segments, ring_count=rings, radius=radius, location=loc)
    obj = bpy.context.active_object
    obj.scale = scale
    return bake(obj)


def box(size, loc=(0, 0, 0), rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc, rotation=rot)
    obj = bpy.context.active_object
    obj.scale = size
    return bake(obj)


def cylinder(radius, depth, loc=(0, 0, 0), rot=(0, 0, 0), vertices=48):
    bpy.ops.mesh.primitive_cylinder_add(vertices=vertices, radius=radius, depth=depth, location=loc, rotation=rot)
    return bake(bpy.context.active_object)


def tube(points, radius, name, cyclic=True, resolution=6):
    """A round tube along a path of points."""
    curve = bpy.data.curves.new(name, "CURVE")
    curve.dimensions = "3D"
    curve.bevel_depth = radius
    curve.bevel_resolution = resolution
    spline = curve.splines.new("POLY")
    spline.points.add(len(points) - 1)
    for point, (x, y, z) in zip(spline.points, points):
        point.co = (x, y, z, 1.0)
    spline.use_cyclic_u = cyclic
    obj = bpy.data.objects.new(name, curve)
    scene().collection.objects.link(obj)
    select_only([obj])
    bpy.ops.object.convert(target="MESH")
    return bpy.context.view_layer.objects.active


def smoothstep(e0, e1, x):
    t = max(0.0, min(1.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


def bump(a, centre, width):
    return math.exp(-(((a - centre) / width) ** 2))


def normalise(objs, height=2.0):
    """Centres the parts together on the origin and scales them so the tallest
    side measures `height`."""
    corners = [obj.matrix_world @ Vector(c) for obj in objs for c in obj.bound_box]
    lo = Vector((min(c[i] for c in corners) for i in range(3)))
    hi = Vector((max(c[i] for c in corners) for i in range(3)))
    centre, size = (lo + hi) / 2, max(hi - lo)
    factor = height / size
    for obj in objs:
        # The scale acts after the move, about the origin, so the move must
        # be scaled too: world = factor * (point - centre).
        obj.location = (obj.location - centre) * factor
        obj.scale *= factor
        bake(obj)


# ------------------------------------------------------------------ models
#
# Blender's axes: Z is up and the front of a model faces -Y (toward the
# camera in the preview). The GLB exporter turns that into Y up, front +Z.


def build_mirrorball():
    """A mirror ball: a sphere of small flat mirror tiles with grooves between
    them, hanging from a rod long enough to run off the top of the screen."""
    ball = sphere(1.0, segments=48, rings=24)
    ball.name = "mirrorball"
    bm = bmesh.new()
    bm.from_mesh(ball.data)
    # Each face becomes a tile, slightly raised, with a sloping edge that
    # reads as the groove between tiles.
    bmesh.ops.inset_individual(bm, faces=list(bm.faces), thickness=0.012, depth=0.006, use_even_offset=True)
    bm.to_mesh(ball.data)
    bm.free()
    cap = cylinder(0.12, 0.16, loc=(0, 0, 1.02), vertices=24)
    rod = cylinder(0.025, 3.0, loc=(0, 0, 2.5), vertices=12)
    hung = join([ball, cap, rod], "mirrorball")
    shade(hung, flat=True)  # flat: every tile is its own little mirror
    normalise([hung])
    return [hung]


# The skull is not built here: it is a CC0 model by Vladimir Petkovic from
# Khronos's glTF sample collection, reduced to a size that renders easily.
# Download it once into blender/sources/ (see README):
# https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models/ScatteringSkull
SKULL_SOURCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "sources", "ScatteringSkull.glb")


def build_skull():
    """An anatomical skull, reduced from 189,000 triangles to about 30,000."""
    if not os.path.exists(SKULL_SOURCE):
        raise FileNotFoundError(f"download the skull first, to {SKULL_SOURCE}")
    bpy.ops.import_scene.gltf(filepath=SKULL_SOURCE)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    for obj in [o for o in bpy.context.scene.objects if o.type != "MESH"]:
        bpy.data.objects.remove(obj, do_unlink=True)  # empties and helpers from the file
    skull = join(meshes, "skull") if len(meshes) > 1 else bake(meshes[0])
    skull.name = "skull"
    skull.data.materials.clear()
    # Soften the finest detail (bone sutures, pores) first: in a mirror
    # finish it breaks reflections into flickering specks.
    smooth(skull, 0.5, 3)
    decimate(skull, 30000)
    shade(skull, flat=False)
    normalise([skull])
    return [skull]


def build_prism():
    """A triangular prism standing on end, its edges just softened."""
    prism = cylinder(0.9, 2.2, vertices=3)
    prism.name = "prism"
    modifier(prism, "BEVEL", width=0.025, segments=2)
    bake(prism)
    shade(prism, flat=True)
    normalise([prism])
    return [prism]


def build_gem():
    """A cut diamond: table, crown, girdle and pavilion, as the hull of points."""
    bm = bmesh.new()
    ring = lambda count, radius, z, turn=0.0: [
        (radius * math.cos(2 * math.pi * (i + turn) / count), radius * math.sin(2 * math.pi * (i + turn) / count), z)
        for i in range(count)
    ]
    points = (
        ring(8, 0.55, 0.38)  # table
        + ring(8, 0.82, 0.22, 0.5)  # star facets
        + ring(16, 1.0, 0.03, 0.25)  # girdle, top
        + ring(16, 1.0, -0.03, 0.25)  # girdle, bottom
        + ring(8, 0.5, -0.55, 0.5)  # pavilion
        + [(0, 0, -1.0)]  # culet
    )
    for p in points:
        bm.verts.new(p)
    hull = bmesh.ops.convex_hull(bm, input=list(bm.verts))
    bmesh.ops.delete(bm, geom=hull["geom_interior"] + hull["geom_unused"], context="VERTS")
    mesh = bpy.data.meshes.new("gem")
    bm.to_mesh(mesh)
    bm.free()
    gem = bpy.data.objects.new("gem", mesh)
    scene().collection.objects.link(gem)
    shade(gem, flat=True)
    normalise([gem])
    return [gem]


def build_knot():
    """A trefoil knot: an endless tube that passes through itself three times."""
    points = []
    for i in range(360):
        t = 2 * math.pi * i / 360
        r = math.cos(3 * t) + 2.2
        # Upright, facing the viewer: the knot's loops lie in the X-Z plane.
        points.append((r * math.cos(2 * t), -1.1 * math.sin(3 * t), r * math.sin(2 * t)))
    knot = tube(points, 0.55, "knot", resolution=8)
    shade(knot, flat=False)
    normalise([knot])
    return [knot]


def build_eye():
    """A big eyeball with a dished iris, ringed, and a glowing pupil."""
    eye = sphere(1.0, segments=96, rings=48)
    eye.name = "eye"
    carve(eye, sphere(0.62, loc=(0, -1.47, 0), segments=96, rings=48))  # the iris dish
    rings = []
    for radius in (0.28, 0.4):
        bpy.ops.mesh.primitive_torus_add(
            major_radius=radius, minor_radius=0.018, location=(0, -0.99 + radius * 0.1, 0),
            rotation=(math.pi / 2, 0, 0), major_segments=96, minor_segments=8,
        )
        rings.append(bake(bpy.context.active_object))
    eye = join([eye] + rings, "eye")
    remesh(eye, 0.012)
    smooth(eye, 0.4, 2)
    decimate(eye, 12000)
    shade(eye, flat=False)
    pupil = sphere(0.17, loc=(0, -0.88, 0), scale=(1, 0.5, 1))
    pupil.name = "eye_glow"
    shade(pupil, flat=False)
    normalise([eye, pupil])
    return [eye, pupil]


def build_monolith():
    """A tall slab split by cracks, with light inside them."""
    random.seed(7)
    slab = box((0.62, 0.26, 2.0))
    slab.name = "monolith"
    modifier(slab, "BEVEL", width=0.015, segments=2)
    bake(slab)

    def crack_path(start, end, steps, wander):
        (x0, z0), (x1, z1) = start, end
        path = []
        for i in range(steps + 1):
            t = i / steps
            jitter = 0 if i in (0, steps) else random.uniform(-wander, wander)
            path.append((x0 + (x1 - x0) * t + jitter, z0 + (z1 - z0) * t))
        return path

    def strip(path, width, depth, name):
        """A thin wall along the path, `depth` thick in Y."""
        bm = bmesh.new()
        prev = None
        for x, z in path:
            pair = (bm.verts.new((x - width, 0, z)), bm.verts.new((x + width, 0, z)))
            if prev:
                bm.faces.new((prev[0], prev[1], pair[1], pair[0]))
            prev = pair
        mesh = bpy.data.meshes.new(name)
        bm.to_mesh(mesh)
        bm.free()
        obj = bpy.data.objects.new(name, mesh)
        scene().collection.objects.link(obj)
        modifier(obj, "SOLIDIFY", thickness=depth, offset=0.0)
        return bake(obj)

    cracks = [
        crack_path((0.05, 1.05), (-0.04, -1.05), 14, 0.09),
        crack_path((0.0, 0.25), (0.34, -0.25), 5, 0.05),
        crack_path((-0.02, -0.4), (-0.34, -0.85), 4, 0.05),
    ]
    glow = []
    for i, path in enumerate(cracks):
        carve(slab, strip(path, 0.022, 0.6, f"cut{i}"))
        glow.append(strip(path, 0.012, 0.2, f"light{i}"))
    light = join(glow, "monolith_glow")
    shade(slab, flat=True)
    shade(light, flat=True)
    normalise([slab, light])
    return [slab, light]


def build_chain():
    """A long chain of eighteen heavy links. noctiluca draws several of them
    at once, long enough to cross the whole screen."""
    links = []
    for i in range(18):
        points = []
        radius, straight = 0.3, 0.32
        for j in range(48):
            a = 2 * math.pi * j / 48
            z = radius * math.sin(a) + math.copysign(straight / 2, math.sin(a))
            points.append((radius * math.cos(a), 0.0, z))
        link = tube(points, 0.12, f"link{i}", resolution=6)
        link.rotation_euler = (0, 0, (math.pi / 2) * (i % 2))
        link.location = (0, 0, -i * 0.78)
        links.append(bake(link))
    chain = join(links, "chain")
    shade(chain, flat=False)
    normalise([chain])
    return [chain]


def cone(radius, depth, cap, name, segments=48, rings=10):
    """A loudspeaker cone facing -Y: its rim at y=0, its middle `depth`
    further back, with a domed dust cap of radius `cap` in the middle.
    noctiluca pushes it forward on the kick: parts named "..._cone" move
    most in their middle and not at all at their rim."""
    bm = bmesh.new()
    rows = []
    for i in range(rings + 1):
        r = radius - (radius - cap) * i / rings
        t = (radius - r) / (radius - cap)
        y = depth * (1 - (1 - t) ** 1.4)          # a curved flare, like a real cone
        rows.append([bm.verts.new((r * math.cos(a), y, r * math.sin(a)))
                     for a in (2 * math.pi * k / segments for k in range(segments))])
    # The dust cap: a dome bulging forward from the cone's throat.
    for i in range(1, 6):
        phi = (math.pi / 2) * i / 6
        r, y = cap * math.cos(phi), depth - cap * 0.6 * math.sin(phi)
        rows.append([bm.verts.new((r * math.cos(a), y, r * math.sin(a)))
                     for a in (2 * math.pi * k / segments for k in range(segments))])
    tip = bm.verts.new((0, depth - cap * 0.6, 0))
    for a, b in zip(rows, rows[1:]):
        for k in range(segments):
            bm.faces.new((a[k], a[(k + 1) % segments], b[(k + 1) % segments], b[k]))
    for k in range(segments):
        bm.faces.new((rows[-1][k], rows[-1][(k + 1) % segments], tip))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new(name, mesh)
    scene().collection.objects.link(obj)
    return obj


def build_speakers():
    """A club loudspeaker: a tall cabinet with a big woofer and a tweeter
    above it, their cones free to pump, and a ring of light round each."""
    width, deep, tall = 1.0, 0.7, 1.6
    front = -deep / 2
    cab = box((width, deep, tall))
    modifier(cab, "BEVEL", width=0.03, segments=3)
    bake(cab)
    parts, glows = [], []
    for name, z, radius in [("woofer", -0.3, 0.4), ("tweeter", 0.48, 0.18)]:
        # A round hole in the front for the driver to sit in.
        carve(cab, cylinder(radius + 0.02, 0.5, loc=(0, front, z), rot=(math.pi / 2, 0, 0), vertices=64))
        c = cone(radius - 0.02, radius * 0.45, radius * 0.28, f"{name}_cone")
        c.location = (0, front + 0.01, z)
        parts.append(bake(c))
        # The rubber surround that holds the cone at its rim.
        bpy.ops.mesh.primitive_torus_add(major_radius=radius - 0.005, minor_radius=radius * 0.06,
                                         location=(0, front, z), rotation=(math.pi / 2, 0, 0),
                                         major_segments=64, minor_segments=12)
        parts.append(bake(bpy.context.active_object))
        # A thin ring of light just outside it.
        bpy.ops.mesh.primitive_torus_add(major_radius=radius + 0.045, minor_radius=0.012,
                                         location=(0, front - 0.005, z), rotation=(math.pi / 2, 0, 0),
                                         major_segments=96, minor_segments=8)
        glows.append(bake(bpy.context.active_object))
    # Behind the drivers the hole would show the box's inside: close it.
    for z, radius in [(-0.3, 0.4), (0.48, 0.18)]:
        parts.append(cylinder(radius + 0.02, 0.02, loc=(0, front + radius * 0.5, z), rot=(math.pi / 2, 0, 0), vertices=64))
    cones = [p for p in parts if p.name.endswith("_cone")]
    body = join([cab] + [p for p in parts if p not in cones], "speakers")
    light = join(glows, "speakers_glow")
    shade(body, flat=False)
    for c in cones:
        shade(c, flat=False)
    shade(light, flat=False)
    objs = [body, light] + cones
    normalise(objs)
    return objs


def build_crystal():
    """One long crystal shard: a six-sided column, a little irregular,
    sharpened to a long point at one end and a short one at the other.
    noctiluca grows a whole crystal out of many of them."""
    random.seed(11)
    bm = bmesh.new()
    sides, radius = 6, 0.27
    ring = lambda z, s: [bm.verts.new(((radius * s + random.uniform(-0.015, 0.015)) * math.cos(a),
                                        (radius * s + random.uniform(-0.015, 0.015)) * math.sin(a), z))
                         for a in (2 * math.pi * k / sides + 0.2 for k in range(sides))]
    low, high = ring(-0.6, 1.0), ring(0.55, 0.95)
    top = bm.verts.new((0.02, -0.01, 1.0))
    bottom = bm.verts.new((0, 0.01, -0.85))
    for k in range(sides):
        n = (k + 1) % sides
        bm.faces.new((low[k], low[n], high[n], high[k]))
        bm.faces.new((high[k], high[n], top))
        bm.faces.new((low[n], low[k], bottom))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new("crystal")
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("crystal", mesh)
    scene().collection.objects.link(obj)
    shade(obj, flat=True)
    normalise([obj])
    return [obj]


def build_planet():
    """A planet with rings of light round it, tilted, like Saturn."""
    planet = sphere(1.0, segments=96, rings=48)
    planet.name = "planet"
    shade(planet, flat=False)
    tilt = (math.radians(90 - 22), 0, math.radians(12))
    bands = []
    # Bands of different widths with gaps between, as real rings have.
    for inner, outer in [(1.3, 1.48), (1.53, 1.72), (1.76, 1.8), (1.85, 1.98)]:
        bpy.ops.mesh.primitive_circle_add(vertices=160, radius=outer, fill_type="NOTHING")
        edge = bpy.context.active_object
        bm = bmesh.new()
        bm.from_mesh(edge.data)
        outer_verts = list(bm.verts)
        ret = bmesh.ops.extrude_edge_only(bm, edges=list(bm.edges))
        inner_verts = [e for e in ret["geom"] if isinstance(e, bmesh.types.BMVert)]
        for v in inner_verts:
            v.co *= inner / outer
        bm.to_mesh(edge.data)
        bm.free()
        modifier(edge, "SOLIDIFY", thickness=0.02, offset=0.0)
        edge.rotation_euler = (math.radians(-22), math.radians(12), 0)
        bands.append(bake(edge))
    rings = join(bands, "planet_glow")
    shade(rings, flat=False)
    normalise([planet, rings])
    return [planet, rings]


def build_corridor():
    """One section of a corridor: a square frame you look through, with
    strips of light along its inner edges. noctiluca lines many of them up
    and flies them toward the camera."""
    outer, inner, deep = 2.0, 1.72, 0.18
    frame = box((outer, deep, outer))
    carve(frame, box((inner, deep * 2, inner)))
    modifier(frame, "BEVEL", width=0.012, segments=2)
    bake(frame)
    shade(frame, flat=True)
    frame.name = "corridor"
    strips = []
    h = inner / 2 - 0.012
    for loc, size in [((0, -deep * 0.2, h), (inner * 0.9, 0.03, 0.025)), ((0, -deep * 0.2, -h), (inner * 0.9, 0.03, 0.025)),
                      ((h, -deep * 0.2, 0), (0.025, 0.03, inner * 0.9)), ((-h, -deep * 0.2, 0), (0.025, 0.03, inner * 0.9))]:
        strips.append(box(size, loc=loc))
    light = join(strips, "corridor_glow")
    shade(light, flat=True)
    normalise([frame, light])
    return [frame, light]


MODELS = {
    "mirrorball": build_mirrorball,
    "skull": build_skull,
    "prism": build_prism,
    "gem": build_gem,
    "knot": build_knot,
    "eye": build_eye,
    "monolith": build_monolith,
    "chain": build_chain,
    "speakers": build_speakers,
    "crystal": build_crystal,
    "planet": build_planet,
    "corridor": build_corridor,
}

# ------------------------------------------------ the 2026-11-22 objects
#
# Two come from free sources, downloaded once into blender/sources/ (kept
# out of the repository) and reduced here:
#   bust       Marble Bust 01 by Rico Cilliers, Poly Haven, CC0:
#              https://polyhaven.com/a/marble_bust_01
#   astronaut  Astronaut, NASA 3D Resources (model by DigitalSpace):
#              https://github.com/nasa/NASA-3D-Resources/tree/master/3D%20Models/Astronaut
# The others are built here.

SOURCES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "sources")


def imported(path, name, faces):
    """A model from a file, as one mesh without its materials, reduced to
    about `faces` faces."""
    if not os.path.exists(path):
        raise FileNotFoundError(f"download it first, to {path}")
    bpy.ops.import_scene.gltf(filepath=path)
    bpy.context.view_layer.update()
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    # Keep where each part really is: a file may stand its model up (or
    # scale it) through a parent, which goes when the helpers are removed.
    for obj in meshes:
        world = obj.matrix_world.copy()
        obj.parent = None
        obj.matrix_world = world
    for obj in [o for o in bpy.context.scene.objects if o.type != "MESH"]:
        bpy.data.objects.remove(obj, do_unlink=True)
    obj = join(meshes, name) if len(meshes) > 1 else bake(meshes[0])
    obj.name = name
    obj.data.materials.clear()
    if len(obj.data.polygons) > faces:
        decimate(obj, faces)
    return obj


def build_bust():
    """A classical marble bust (Poly Haven, CC0)."""
    bust = imported(os.path.join(SOURCES, "marble_bust", "marble_bust_01_1k.gltf"), "bust", 16000)
    smooth(bust, 0.4, 2)
    shade(bust, flat=False)
    normalise([bust])
    return [bust]


def build_astronaut():
    """An astronaut in a spacesuit (NASA 3D Resources)."""
    man = imported(os.path.join(SOURCES, "astronaut", "Astronaut.glb"), "astronaut", 12000)
    shade(man, flat=False)
    normalise([man])
    return [man]


def build_diamond():
    """A round brilliant, cut to the classic proportions: an eight-sided
    table, star and bezel facets on the crown, a sixteen-sided girdle and a
    pavilion down to the culet; the hull of those points, every facet flat."""
    points = []
    for k in range(8):  # table
        a = (k + 0.5) * math.tau / 8
        points.append((0.55 * math.cos(a), 0.55 * math.sin(a), 0.16))
    for k in range(8):  # bezel points, between the table's corners
        a = k * math.tau / 8
        points.append((0.80 * math.cos(a), 0.80 * math.sin(a), 0.085))
    for k in range(16):  # girdle, top and bottom edge
        a = (k + 0.5) * math.tau / 16
        points.append((math.cos(a), math.sin(a), 0.01))
        points.append((math.cos(a), math.sin(a), -0.01))
    for k in range(8):  # pavilion mains
        a = (k + 0.5) * math.tau / 8
        points.append((0.42 * math.cos(a), 0.42 * math.sin(a), -0.27))
    points.append((0, 0, -0.43))  # culet
    mesh = bpy.data.meshes.new("diamond")
    bm = bmesh.new()
    for p in points:
        bm.verts.new(p)
    bmesh.ops.convex_hull(bm, input=list(bm.verts))
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("diamond", mesh)
    scene().collection.objects.link(obj)
    shade(obj, flat=True)
    normalise([obj])
    return [obj]


def build_vinyl():
    """A record on a platter: grooves as fine rings, a label that glows in
    the palette's colour, the spindle, and a tone arm resting in the groove."""
    disc = cylinder(1.0, 0.02, vertices=96)
    disc.name = "vinyl"
    bm = bmesh.new()
    bm.from_mesh(disc.data)
    top = [f for f in bm.faces if f.normal.z > 0.9]
    # Rings of grooves from the label out to the edge.
    for k in range(34):
        # Each inset shrinks the same inner face again, a little up or down.
        bmesh.ops.inset_region(bm, faces=top, thickness=0.018, depth=0.0015 if k % 2 else -0.0015)
    bm.to_mesh(disc.data)
    bm.free()
    label = cylinder(0.34, 0.026, vertices=64)
    label.name = "vinyl_label_glow"
    spindle = cylinder(0.025, 0.09, loc=(0, 0, 0.04), vertices=16)
    platter = cylinder(1.04, 0.06, loc=(0, 0, -0.045), vertices=96)
    base = box((0.18, 0.18, 0.1), loc=(1.18, 0.75, 0.0))
    arm = tube([(1.18, 0.75, 0.08), (1.05, 0.2, 0.1), (0.62, -0.18, 0.08)], 0.018, "arm", cyclic=False)
    head = box((0.08, 0.14, 0.03), loc=(0.6, -0.22, 0.05), rot=(0, 0, 0.6))
    deck = join([disc, spindle, platter, base, arm, head], "vinyl")
    shade(deck, flat=False)
    normalise([deck, label])
    return [deck, label]


def build_jellyfish():
    """A jellyfish: a bell with a ruffled rim that glows, four frilled oral
    arms and long trailing tentacles."""
    bell = sphere(1.0, segments=64, rings=32)
    bell.name = "jellyfish"
    bm = bmesh.new()
    bm.from_mesh(bell.data)
    for v in bm.verts:
        x, y, z = v.co
        if z < -0.05:
            # The underside is open and hollow: the lower half folds up inside.
            z = -0.05 + (z + 0.05) * -0.35
        a = math.atan2(y, x)
        ruffle = 1.0 + 0.05 * math.sin(a * 16) * smoothstep(0.3, -0.05, z)
        v.co = (x * ruffle, y * ruffle, z * 0.75)
    bm.to_mesh(bell.data)
    bm.free()
    rim = tube([(math.cos(a) * 1.0, math.sin(a) * 1.0, -0.04 + 0.03 * math.sin(a * 16)) for a in [k * math.tau / 96 for k in range(96)]], 0.035, "jellyfish_rim_glow")
    parts = [bell]
    rng = random.Random(7)
    for k in range(4):  # oral arms: thick, wavy, near the middle
        a = k * math.tau / 4 + 0.4
        pts = [(0.25 * math.cos(a) + 0.12 * math.sin(t * 1.7 + k), 0.25 * math.sin(a) + 0.12 * math.cos(t * 1.3 + k), -0.1 - t * 0.55) for t in [i * 0.25 for i in range(13)]]
        parts.append(tube(pts, 0.06, f"arm{k}", cyclic=False))
    for k in range(16):  # tentacles: thin and long, from the rim
        a = k * math.tau / 16 + rng.uniform(-0.1, 0.1)
        sway = rng.uniform(0, math.tau)
        pts = [(0.92 * math.cos(a) * (1 - t * 0.04) + 0.08 * math.sin(t * 1.1 + sway), 0.92 * math.sin(a) * (1 - t * 0.04) + 0.08 * math.cos(t * 0.9 + sway), -0.05 - t * 0.6) for t in [i * 0.4 for i in range(16)]]
        parts.append(tube(pts, 0.012, f"tentacle{k}", cyclic=False, resolution=4))
    body = join(parts, "jellyfish")
    shade(body, flat=False)
    normalise([body, rim])
    return [body, rim]


def build_rose():
    """A rose in full bloom: cupped petals in a spiral, small and tight in
    the heart, opening wider outward, on a short stem with two leaves."""
    petals = []
    count = 34
    for k in range(count):
        t = k / (count - 1)
        petal = sphere(1.0, segments=24, rings=16)
        bm = bmesh.new()
        bm.from_mesh(petal.data)
        for v in bm.verts:
            x, y, z = v.co
            # A thin cupped shell: keep the half facing out, curl its lip.
            y = abs(y) * 0.18 + 0.02
            lip = smoothstep(0.2, 1.0, z) * (0.25 + 0.5 * t)
            v.co = (x * 0.55, y + lip * 0.5, z * 0.75 + 0.75)
        bm.to_mesh(petal.data)
        bm.free()
        size = 0.25 + 0.75 * t
        petal.scale = (size, size, size * (1.0 - 0.25 * t))
        petal.rotation_euler = (math.radians(10 + 55 * t), 0, k * math.radians(137.5))
        petal.location = (0, 0, -0.25 * t)
        bake(petal)
        petals.append(petal)
    stem = tube([(0, 0, -0.2), (0.05, 0, -1.0), (0.0, 0.03, -1.9)], 0.05, "stem", cyclic=False)
    leaves = []
    for side in (-1, 1):
        leaf = sphere(0.3, segments=16, rings=8)
        leaf.scale = (1.0, 0.12, 0.45)
        leaf.rotation_euler = (0, math.radians(50 * side), 0)
        leaf.location = (0.3 * side, 0, -1.0 + 0.2 * side)
        bake(leaf)
        leaves.append(leaf)
    rose = join(petals + [stem] + leaves, "rose")
    shade(rose, flat=False)
    normalise([rose])
    return [rose]


def build_blob():
    """A drop of liquid metal: three spheres melted into one smooth body,
    caught mid-wobble."""
    bpy.ops.object.metaball_add(type="BALL", radius=1.0, location=(0, 0, 0))
    ball = bpy.context.active_object
    meta = ball.data
    meta.resolution = 0.06
    meta.render_resolution = 0.06
    for (x, y, z, r) in [(0.55, 0.1, 0.25, 0.75), (-0.5, -0.15, -0.1, 0.8), (0.05, 0.35, -0.55, 0.6), (-0.1, -0.3, 0.6, 0.5)]:
        e = meta.elements.new()
        e.co = (x, y, z)
        e.radius = r
    bpy.ops.object.convert(target="MESH")
    blob = bpy.context.active_object
    blob.name = "blob"
    smooth(blob, 0.5, 6)
    decimate(blob, 14000)
    shade(blob, flat=False)
    normalise([blob])
    return [blob]


def build_pyramid():
    """A pyramid of the Giza proportions, its edges softened."""
    bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1.0, radius2=0.0, depth=0.9, location=(0, 0, 0))
    pyr = bpy.context.active_object
    pyr.name = "pyramid"
    pyr.rotation_euler = (0, 0, math.radians(45))
    bake(pyr)
    modifier(pyr, "BEVEL", width=0.01, segments=2)
    bake(pyr)
    shade(pyr, flat=True)
    normalise([pyr])
    return [pyr]


def build_moon():
    """The moon: a sphere pitted with craters of every size, each a dish
    with a raised rim."""
    moon = sphere(1.0, segments=128, rings=64)
    moon.name = "moon"
    rng = random.Random(11)
    craters = []
    for _ in range(170):
        d = Vector((rng.gauss(0, 1), rng.gauss(0, 1), rng.gauss(0, 1))).normalized()
        craters.append((d, rng.uniform(0.03, 0.2) ** 1.5 * 1.4))
    bm = bmesh.new()
    bm.from_mesh(moon.data)
    for v in bm.verts:
        n = v.co.normalized()
        h = 0.0
        for d, r in craters:
            x = math.acos(max(-1.0, min(1.0, n.dot(d)))) / r
            if x < 1.6:
                h += r * 0.1 * (-(1 - x * x) if x < 1 else 0.35 * math.exp(-((x - 1.05) / 0.2) ** 2))
        v.co = n * (1.0 + h)
    bm.to_mesh(moon.data)
    bm.free()
    shade(moon, flat=False)
    normalise([moon])
    return [moon]


def build_dna():
    """A DNA double helix: two backbones winding round each other, joined
    by base pairs, the pairs' middles glowing."""
    turns, height, radius = 3.0, 4.0, 0.45
    parts, glow = [], []
    for side in (0, 1):
        pts = []
        for i in range(181):
            t = i / 180
            a = t * turns * math.tau + side * math.pi
            pts.append((radius * math.cos(a), radius * math.sin(a), (t - 0.5) * height))
        parts.append(tube(pts, 0.05, f"backbone{side}", cyclic=False, resolution=8))
    for k in range(30):
        t = (k + 0.5) / 30
        a = t * turns * math.tau
        z = (t - 0.5) * height
        p0 = Vector((radius * math.cos(a), radius * math.sin(a), z))
        p1 = -Vector((p0.x, p0.y, 0)) + Vector((0, 0, z))
        mid = (p0 + p1) / 2
        rung = cylinder(0.022, (p1 - p0).length * 0.9, loc=mid, vertices=10)
        rung.rotation_euler = (math.pi / 2, 0, a + math.pi / 2)
        bake(rung)
        parts.append(rung)
        light = sphere(0.045, loc=mid, segments=12, rings=6)
        glow.append(light)
    helix = join(parts, "dna")
    lights = join(glow, "dna_pairs_glow")
    shade(helix, flat=False)
    normalise([helix, lights])
    return [helix, lights]


def build_butterfly():
    """A butterfly with its wings open: fore and hind wings on each side,
    gently bent, and a slim body."""
    wings = []
    for side in (-1, 1):
        for (w, h, lift, back) in [(1.0, 0.75, 0.25, 0.1), (0.7, 0.6, -0.35, -0.05)]:
            wing = sphere(0.5, segments=24, rings=12)
            wing.scale = (w, h, 0.02)
            wing.location = (side * (w * 0.5 + 0.06), back, lift)
            wing.rotation_euler = (0, math.radians(-12 * side), math.radians(18 * side * (1 if lift > 0 else -1)))
            bake(wing)
            wings.append(wing)
    body = sphere(0.06, segments=16, rings=12)
    body.scale = (1.0, 1.0, 8.0)
    bake(body)
    fly = join(wings + [body], "butterfly")
    shade(fly, flat=False)
    normalise([fly])
    return [fly]


def build_cubes():
    """One cube of the swarm, its edges bevelled to catch the light."""
    cube = box((1.0, 1.0, 1.0))
    cube.name = "cubes"
    modifier(cube, "BEVEL", width=0.06, segments=3)
    bake(cube)
    shade(cube, flat=False)
    normalise([cube])
    return [cube]


MODELS.update({
    "bust": build_bust,
    "diamond": build_diamond,
    "vinyl": build_vinyl,
    "jellyfish": build_jellyfish,
    "rose": build_rose,
    "blob": build_blob,
    "pyramid": build_pyramid,
    "moon": build_moon,
    "dna": build_dna,
    "butterfly": build_butterfly,
    "cubes": build_cubes,
    "astronaut": build_astronaut,
})

# ------------------------------------------------------------ export, render


def export(objs, path):
    select_only(objs)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_materials="NONE",
        export_normals=True,
        export_yup=True,
    )


def material(name, **inputs):
    mat = bpy.data.materials.new(name)
    node = mat.node_tree.nodes["Principled BSDF"]
    for key, value in inputs.items():
        node.inputs[key].default_value = value
    return mat


def stage():
    """A dark club corner: red neon tubes behind, one white tube in front for
    highlights, and a glossy black floor."""
    s = scene()
    s.render.engine = "CYCLES"
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "METAL"
        prefs.refresh_devices()
        for device in prefs.devices:
            device.use = True
        s.cycles.device = "GPU"
    except Exception as e:  # no GPU: Cycles falls back to the CPU
        print("rendering on the CPU:", e)
    s.cycles.samples = 64
    s.cycles.use_denoising = True
    s.render.resolution_x = s.render.resolution_y = 640
    s.world = bpy.data.worlds.new("dark")
    s.world.use_nodes = True
    background = s.world.node_tree.nodes["Background"]
    background.inputs["Color"].default_value = (0, 0, 0, 1)
    background.inputs["Strength"].default_value = 0.0

    red = material("neon_red", **{"Base Color": (0, 0, 0, 1), "Emission Color": (1, 0.04, 0.02, 1), "Emission Strength": 12.0})
    white = material("neon_white", **{"Base Color": (0, 0, 0, 1), "Emission Color": (1, 1, 1, 1), "Emission Strength": 6.0})
    floor = material("floor", **{"Base Color": (0.01, 0.01, 0.012, 1), "Roughness": 0.12})
    rig = []
    for x in (-2.4, -1.2, 1.2, 2.4):
        rig.append(cylinder(0.04, 4.0, loc=(x, 2.2, 0.6), vertices=16))
        rig[-1].data.materials.append(red)
    rig.append(cylinder(0.04, 4.0, loc=(0, -3.2, 2.6), rot=(0, math.pi / 2, 0), vertices=16))
    rig[-1].data.materials.append(white)
    rig.append(cylinder(0.04, 3.0, loc=(-2.6, -1.6, 0.3), vertices=16))  # a tube to the left, for an edge
    rig[-1].data.materials.append(white)
    # A soft box above and in front: the big shape chrome needs to show its form.
    soft = material("softbox", **{"Base Color": (0, 0, 0, 1), "Emission Color": (1, 1, 1, 1), "Emission Strength": 5.0})
    bpy.ops.mesh.primitive_plane_add(size=1, location=(0.8, -2.6, 2.4), rotation=(math.radians(-50), 0, math.radians(15)))
    rig.append(bpy.context.active_object)
    rig[-1].scale = (2.5, 1.2, 1)
    rig[-1].data.materials.append(soft)
    bpy.ops.mesh.primitive_plane_add(size=1, location=(-2.2, 1.2, 1.2), rotation=(math.radians(90), 0, math.radians(-60)))
    rig.append(bpy.context.active_object)  # a second, dimmer one behind on the left, for a rim
    rig[-1].scale = (1.2, 2.5, 1)
    rig[-1].data.materials.append(soft)
    bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, -1.02))
    rig.append(bpy.context.active_object)
    rig[-1].data.materials.append(floor)

    cam = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    cam.data.lens = 70
    cam.location = (0, -5.2, 0.25)
    cam.rotation_euler = (math.radians(88), 0, 0)
    s.collection.objects.link(cam)
    s.camera = cam
    return rig


def render(objs, path, kind):
    chrome = material("chrome", **{"Base Color": (0.95, 0.95, 0.97, 1), "Metallic": 1.0, "Roughness": 0.05})
    glass = material(
        "glass", **{"Base Color": (0.96, 0.98, 1.0, 1), "Roughness": 0.0, "IOR": 1.5, "Transmission Weight": 1.0}
    )
    glow = material("glow", **{"Base Color": (0, 0, 0, 1), "Emission Color": (1, 0.1, 0.05, 1), "Emission Strength": 15.0})
    for obj in objs:
        obj.data.materials.clear()
        if obj.name.endswith("_glow"):
            obj.data.materials.append(glow)
        else:
            obj.data.materials.append(chrome if kind == "chrome" else glass)
    # A three-quarter view shows the form better than straight on; the eye
    # is the exception, it is meant to look at you.
    for obj in objs:
        obj.rotation_euler = (0, 0, 0 if objs[0].name == "eye" else math.radians(-28))
    scene().render.filepath = path
    bpy.ops.render.render(write_still=True)


def main():
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if len(args) < 2:
        sys.exit("usage: ... -- MODELS_DIR PREVIEW_DIR|- [names]")
    models_dir, preview_dir, only = args[0], args[1], args[2:]
    os.makedirs(models_dir, exist_ok=True)
    if preview_dir != "-":
        os.makedirs(preview_dir, exist_ok=True)

    for name, build in MODELS.items():
        if only and name not in only:
            continue
        clear()
        objs = build()
        faces = sum(len(o.data.polygons) for o in objs)
        path = os.path.join(models_dir, f"{name}.glb")
        export(objs, path)
        print(f"MODEL {name}: {faces} faces, {os.path.getsize(path) // 1024} KB")
        if preview_dir != "-":
            stage()
            for kind in ("chrome", "glass"):
                render(objs, os.path.join(preview_dir, f"{name}_{kind}.png"), kind)


main()
