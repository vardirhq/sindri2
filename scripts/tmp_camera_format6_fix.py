from pathlib import Path
p = Path('crates/sindri-core/src/migration.rs')
s = p.read_text()
old = '''        let rotate = |v: [f64;3]| {
            let [x,y,z,w] = q;
            let u = [x,y,z];
            let uv = cross(u, v);
            let uuv = cross(u, uv);
            [v[0] + 2.0*(w*uv[0]+uuv[0]), v[1] + 2.0*(w*uv[1]+uuv[1]), v[2] + 2.0*(w*uv[2]+uuv[2])]
        };
        let forward = normalize([-3.0,-2.0,-4.0]).unwrap();
        let actual = rotate([0.0,0.0,-1.0]);
        assert!(dot(sub(actual, forward), sub(actual, forward)) < 1.0e-12);
'''
new = '''        let cross = |a: [f64;3], b: [f64;3]| [
            a[1]*b[2]-a[2]*b[1],
            a[2]*b[0]-a[0]*b[2],
            a[0]*b[1]-a[1]*b[0],
        ];
        let rotate = |v: [f64;3]| {
            let [x,y,z,w] = q;
            let u = [x,y,z];
            let uv = cross(u, v);
            let uuv = cross(u, uv);
            [v[0] + 2.0*(w*uv[0]+uuv[0]), v[1] + 2.0*(w*uv[1]+uuv[1]), v[2] + 2.0*(w*uv[2]+uuv[2])]
        };
        let length = 29.0_f64.sqrt();
        let forward = [-3.0/length, -2.0/length, -4.0/length];
        let actual = rotate([0.0,0.0,-1.0]);
        let error = [actual[0]-forward[0], actual[1]-forward[1], actual[2]-forward[2]];
        assert!(error[0]*error[0] + error[1]*error[1] + error[2]*error[2] < 1.0e-12);
'''
if old not in s:
    raise SystemExit('test block not found')
p.write_text(s.replace(old, new))
