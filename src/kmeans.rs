use std::collections::HashMap;

use glam::Vec3;
use rand::Rng;

use crate::{
    bsp::{Polygon, Quad, Triangle},
    MeshGridEntry,
};

pub fn kmeans_cluster(
    mesh_map: HashMap<String, MeshGridEntry>,
    poly_limit: u32,
) -> Vec<MeshGridEntry> {
    let mut polygons = Vec::<Polygon>::new();

    // Get all polygons in one big buffer
    let mut i = 0;
    for value in mesh_map.values() {
        for tri in value.triangles.chunks(3) {
            polygons.push(Polygon::Triangle(
                i,
                Triangle {
                    v0: Vec3 {
                        x: tri[0].pos_x as f32,
                        y: tri[0].pos_y as f32,
                        z: tri[0].pos_z as f32,
                    },
                    v1: Vec3 {
                        x: tri[1].pos_x as f32,
                        y: tri[1].pos_y as f32,
                        z: tri[1].pos_z as f32,
                    },
                    v2: Vec3 {
                        x: tri[2].pos_x as f32,
                        y: tri[2].pos_y as f32,
                        z: tri[2].pos_z as f32,
                    },
                },
                tri,
            ));
            i += 3;
        }
        for quad in value.quads.chunks(4) {
            polygons.push(Polygon::Quad(
                i,
                Quad {
                    v0: Vec3 {
                        x: quad[0].pos_x as f32,
                        y: quad[0].pos_y as f32,
                        z: quad[0].pos_z as f32,
                    },
                    v1: Vec3 {
                        x: quad[1].pos_x as f32,
                        y: quad[1].pos_y as f32,
                        z: quad[1].pos_z as f32,
                    },
                    v2: Vec3 {
                        x: quad[2].pos_x as f32,
                        y: quad[2].pos_y as f32,
                        z: quad[2].pos_z as f32,
                    },
                    v3: Vec3 {
                        x: quad[3].pos_x as f32,
                        y: quad[3].pos_y as f32,
                        z: quad[3].pos_z as f32,
                    },
                },
                quad,
            ));
            i += 4;
        }
    }

    // How many clusters do we want?
    let n_clusters = polygons.len() / poly_limit as usize;

    // Make the clusters based on random polygon centers
    let mut rng = rand::thread_rng();
    let mut clusters = Vec::new();
    while clusters.len() < n_clusters {
        let pos_to_try = match polygons[rng.gen::<usize>() % polygons.len()] {
            Polygon::Quad(_, quad, _) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 4.0,
            Polygon::Triangle(_, tri, _) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
        };
        if !clusters.contains(&pos_to_try) {
            clusters.push(pos_to_try);
        }
    }

    loop {
        let mut change = false;

        // Assign polygons to clusters
        for polygon in &mut polygons {
            let mut min_distance = f32::INFINITY;
            let index_before = *match polygon {
                Polygon::Triangle(index, _, _) => index,
                Polygon::Quad(index, _, _) => index,
            };
            for (i, pos_cluster) in clusters.iter().enumerate() {
                let pos_polygon = match polygon {
                    Polygon::Quad(_, quad, _) => (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 4.0,
                    Polygon::Triangle(_, tri, _) => (tri.v0 + tri.v1 + tri.v2) / 3.0,
                };

                let distance = pos_cluster.distance(pos_polygon);
                if distance < min_distance {
                    min_distance = distance;
                    match polygon {
                        Polygon::Triangle(index, _, _) => *index = i as _,
                        Polygon::Quad(index, _, _) => *index = i as _,
                    }
                }
            }

            let index_after = *match polygon {
                Polygon::Triangle(index, _, _) => index,
                Polygon::Quad(index, _, _) => index,
            };

            if index_before != index_after {
                change = true;
            }
        }

        // Move clusters to polygons
        for i in 0..clusters.len() {
            let mut acc_center = Vec3::ZERO;
            let mut acc_num = 0;
            for polygon in &polygons {
                let (index, center) = match polygon {
                    Polygon::Quad(index, quad, _) => {
                        (index, (quad.v0 + quad.v1 + quad.v2 + quad.v3) / 4.0)
                    }
                    Polygon::Triangle(index, tri, _) => (index, (tri.v0 + tri.v1 + tri.v2) / 3.0),
                };
                if i == *index as usize {
                    acc_center += center;
                    acc_num += 1;
                }
            }
            acc_center /= acc_num as f32;
        }

        if !change {
            break;
        }
    }

    // Create meshes from the clusters
    let mut meshes = Vec::<MeshGridEntry>::new();
    for i in 0..clusters.len() {
        // Create new mesh
        let mut mesh = MeshGridEntry {
            triangles: Vec::new(),
            quads: Vec::new(),
        };

        // Collect all polygons in this cluster
        for polygon in &polygons {
            match polygon {
                Polygon::Triangle(index, _, verts) => {
                    if *index == i as u32 {
                        mesh.triangles.extend_from_slice(verts)
                    }
                }
                Polygon::Quad(index, _, verts) => {
                    if *index == i as u32 {
                        mesh.quads.extend_from_slice(verts)
                    }
                }
            }
        }

        // Add it
        meshes.push(mesh)
    }

    meshes
}
