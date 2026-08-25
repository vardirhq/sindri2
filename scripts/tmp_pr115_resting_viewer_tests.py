from pathlib import Path

path = Path("crates/sindri-scene/tests/extraction.rs")
text = path.read_text()

old = '''    // The authored camera is at (3, 2, 4), so the western sprite is further.\n    let authored = alphas(CameraView::default());\n    assert!(close(authored[0], 0.75) && close(authored[1], 0.25));\n'''
new = '''    // The resting viewer camera is at (3, 2, 4), so the western sprite is further.\n    let resting = alphas(CameraView {\n        projection: WorldProjection::Perspective,\n        ..CameraView::default()\n    });\n    assert!(close(resting[0], 0.75) && close(resting[1], 0.25));\n'''
if text.count(old) != 1:
    raise SystemExit(f"expected one sprite baseline block, found {text.count(old)}")
text = text.replace(old, new, 1)

old = '''    let resting = extractor\n        .world_camera(&world, CameraView::default())\n        .expect("the world holds a perspective camera")\n        .expect("a perspective camera resolves")\n        .view;\n    // The authored camera sits at (3, 2, 4) looking at the origin, so world X\n'''
new = '''    let resting = extractor\n        .world_camera(\n            &world,\n            CameraView {\n                projection: WorldProjection::Perspective,\n                ..CameraView::default()\n            },\n        )\n        .expect("the viewer has a perspective camera")\n        .expect("a perspective camera resolves")\n        .view;\n    // The resting viewer sits at (3, 2, 4) looking at the origin, so world X\n'''
if text.count(old) != 1:
    raise SystemExit(f"expected one axis baseline block, found {text.count(old)}")
text = text.replace(old, new, 1)

path.write_text(text)
